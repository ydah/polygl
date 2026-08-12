use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::{BuildOptions, BuildPlan, CliError, build, build_plan, resolve_build_request};

const WATCH_INTERVAL: Duration = Duration::from_millis(150);
const FAILED_BUILD_RETRY_INTERVAL: Duration = Duration::from_secs(1);
const ACCEPT_RETRY: Duration = Duration::from_millis(15);
const CLIENT_WRITE_TIMEOUT: Duration = Duration::from_millis(100);
const HTTP_READ_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_REQUEST_BYTES: usize = 16 * 1024;
const MAX_CLIENT_FRAME_BYTES: usize = 64 * 1024;
const MAX_WEBSOCKET_CLIENTS: usize = 32;
const MAX_HTTP_CONNECTIONS: usize = 64;
const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
const CLIENT_START: &str = "<!-- polygl-dev-client:start -->";
const CLIENT_END: &str = "<!-- polygl-dev-client:end -->";
const ERROR_START: &str = "<!-- polygl-build-error:start -->";
const ERROR_END: &str = "<!-- polygl-build-error:end -->";
const INITIAL_ERROR_ID: &str = "polygl-initial-build-error";
const DEV_CLIENT: &str = r##"<script>
(() => {
  const overlayId = "polygl-build-error-overlay";
  const showError = (message) => {
    let overlay = document.getElementById(overlayId);
    if (overlay === null) {
      overlay = document.createElement("pre");
      overlay.id = overlayId;
      overlay.setAttribute("role", "alert");
      Object.assign(overlay.style, {
        background: "rgba(24, 8, 12, 0.96)",
        border: "1px solid #ff6b81",
        boxSizing: "border-box",
        color: "#fff1f3",
        font: "13px/1.5 ui-monospace, SFMono-Regular, Menlo, monospace",
        inset: "0",
        margin: "0",
        overflow: "auto",
        padding: "20px",
        position: "fixed",
        whiteSpace: "pre-wrap",
        zIndex: "2147483647",
      });
      document.body.append(overlay);
    }
    overlay.textContent = message;
  };
  const initial = document.getElementById("polygl-initial-build-error");
  if (initial !== null) {
    showError(initial.textContent);
  }
  let retry = 0;
  const connect = () => {
    const protocol = location.protocol === "https:" ? "wss:" : "ws:";
    const socket = new WebSocket(`${protocol}//${location.host}/__polygl_ws`);
    socket.addEventListener("open", () => { retry = 0; });
    socket.addEventListener("message", (event) => {
      const message = JSON.parse(event.data);
      if (message.type === "reload") {
        location.reload();
      } else if (message.type === "error") {
        showError(message.message);
      }
    });
    socket.addEventListener("close", () => {
      const base = Math.min(10_000, 250 * (2 ** Math.min(retry, 6)));
      retry += 1;
      setTimeout(connect, base + Math.random() * base * 0.25);
    });
  };
  connect();
})();
</script>"##;

static NEXT_GENERATION: AtomicU64 = AtomicU64::new(0);
static NEXT_CLIENT: AtomicU64 = AtomicU64::new(0);

type ActiveGeneration = Arc<RwLock<Option<Arc<Generation>>>>;
type CurrentError = Arc<RwLock<Option<String>>>;
type Clients = Arc<Mutex<Vec<WebSocketClient>>>;
type BroadcastSender = Sender<String>;

struct Generation {
    root: PathBuf,
    watched_paths: Vec<PathBuf>,
}

impl Generation {
    fn build(source: &Path, messages: &mut dyn Write) -> Result<Arc<Self>, CliError> {
        let sequence = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "polygl-serve-{}-{timestamp}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).map_err(|error| {
            CliError::new(format!(
                "failed to create development generation {}: {error}",
                root.display()
            ))
        })?;
        match build(source, &root, BuildOptions::development(), messages) {
            Ok(report) => Ok(Arc::new(Self {
                root,
                watched_paths: report.watched_paths,
            })),
            Err(error) => {
                let _ = fs::remove_dir_all(&root);
                Err(error)
            }
        }
    }
}

impl Drop for Generation {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct ServerState {
    active: ActiveGeneration,
    error: CurrentError,
    clients: Clients,
    broadcaster: BroadcastSender,
    active_connections: AtomicUsize,
    port: u16,
}

#[derive(Clone)]
struct WebSocketClient {
    id: u64,
    writer: Arc<Mutex<TcpStream>>,
}

pub(crate) fn serve(
    source: &Path,
    watch: bool,
    port: u16,
    open: bool,
    messages: &mut dyn Write,
) -> Result<(), CliError> {
    // Capture before compiling. If the file changes during compilation, the
    // first watch iteration observes a different fingerprint and rebuilds it.
    let source_before_build = path_fingerprint(source);
    let (generation, initial_error) = match Generation::build(source, messages) {
        Ok(generation) => (Some(generation), None),
        Err(error) if !watch => return Err(error),
        Err(error) => {
            let message = error.to_string();
            write_error(messages, &message)?;
            (None, Some(message))
        }
    };

    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let listener = TcpListener::bind(address)
        .map_err(|error| CliError::new(format!("failed to bind development server: {error}")))?;
    listener.set_nonblocking(true).map_err(|error| {
        CliError::new(format!("failed to configure development server: {error}"))
    })?;
    let actual_address = listener
        .local_addr()
        .map_err(|error| CliError::new(format!("failed to read server address: {error}")))?;
    writeln!(messages, "serving http://{actual_address}")
        .map_err(|error| CliError::new(format!("failed to write server address: {error}")))?;
    if open {
        open_browser(&format!("http://{actual_address}"))?;
    }
    if initial_error.is_some() {
        writeln!(messages, "waiting for a successful rebuild")
            .map_err(|error| CliError::new(format!("failed to write server status: {error}")))?;
    }

    let clients = Arc::new(Mutex::new(Vec::new()));
    let broadcaster = start_broadcast_worker(Arc::clone(&clients))?;
    let state = Arc::new(ServerState {
        active: Arc::new(RwLock::new(generation)),
        error: Arc::new(RwLock::new(initial_error)),
        clients,
        broadcaster,
        active_connections: AtomicUsize::new(0),
        port: actual_address.port(),
    });
    let mut watched_paths = current_watch_paths(&state, source)?;
    let mut fingerprint = paths_fingerprint(&watched_paths);
    if path_fingerprint(source) != source_before_build {
        fingerprint = None;
    }
    let mut next_watch = Instant::now() + WATCH_INTERVAL;
    let mut next_failed_retry = Instant::now();
    loop {
        accept_pending(&listener, &state)?;
        if watch && Instant::now() >= next_watch {
            next_watch = Instant::now() + WATCH_INTERVAL;
            let observed = paths_fingerprint(&watched_paths);
            let retry_failed_build = state
                .error
                .read()
                .map_err(|_| CliError::new("build error lock is poisoned"))?
                .is_some()
                && Instant::now() >= next_failed_retry;
            if observed != fingerprint || retry_failed_build {
                // Advance to the version this build is intended to consume.
                // A concurrent save changes the next observed value and cannot
                // be swallowed as the new baseline.
                fingerprint = observed;
                next_failed_retry = Instant::now() + FAILED_BUILD_RETRY_INTERVAL;
                match Generation::build(source, messages) {
                    Ok(generation) => {
                        watched_paths = generation.watched_paths.clone();
                        fingerprint = paths_fingerprint(&watched_paths);
                        *state
                            .active
                            .write()
                            .map_err(|_| CliError::new("generation lock is poisoned"))? =
                            Some(generation);
                        *state
                            .error
                            .write()
                            .map_err(|_| CliError::new("build error lock is poisoned"))? = None;
                        broadcast(&state.broadcaster, r#"{"type":"reload"}"#);
                        writeln!(messages, "rebuilt {}", source.display()).map_err(|error| {
                            CliError::new(format!("failed to write rebuild status: {error}"))
                        })?;
                    }
                    Err(error) => {
                        let message = error.to_string();
                        let mut current_error = state
                            .error
                            .write()
                            .map_err(|_| CliError::new("build error lock is poisoned"))?;
                        let changed = current_error.as_deref() != Some(&message);
                        if changed {
                            *current_error = Some(message.clone());
                        }
                        drop(current_error);
                        if changed {
                            write_error(messages, &message)?;
                            let payload = serde_json::json!({
                                "type": "error",
                                "message": message,
                            })
                            .to_string();
                            broadcast(&state.broadcaster, &payload);
                        }
                    }
                }
            }
        }
        thread::sleep(ACCEPT_RETRY);
    }
}

pub(crate) fn watch_build(mut plan: BuildPlan, messages: &mut dyn Write) -> Result<(), CliError> {
    let source_before_build = path_fingerprint(&plan.source);
    let mut watched_paths = configured_watch_paths(&plan);
    let mut fingerprint = paths_fingerprint(&watched_paths);
    let mut failed = false;
    match build_plan(&plan, messages) {
        Ok(report) => {
            watched_paths = report.watched_paths;
            fingerprint = paths_fingerprint(&watched_paths);
            writeln!(messages, "watching {}", plan.source.display())
                .map_err(|error| CliError::new(format!("failed to write watch status: {error}")))?;
        }
        Err(error) => {
            write_error(messages, &error.to_string())?;
            failed = true;
        }
    }
    if path_fingerprint(&plan.source) != source_before_build {
        fingerprint = None;
    }
    let mut next_failed_retry = Instant::now() + FAILED_BUILD_RETRY_INTERVAL;
    loop {
        thread::sleep(WATCH_INTERVAL);
        let observed = paths_fingerprint(&watched_paths);
        if observed == fingerprint && !(failed && Instant::now() >= next_failed_retry) {
            continue;
        }
        fingerprint = observed;
        next_failed_retry = Instant::now() + FAILED_BUILD_RETRY_INTERVAL;
        plan = match resolve_build_request(plan.request.clone()) {
            Ok(plan) => plan,
            Err(error) => {
                write_error(messages, &error.to_string())?;
                failed = true;
                continue;
            }
        };
        match build_plan(&plan, messages) {
            Ok(report) => {
                watched_paths = report.watched_paths;
                fingerprint = paths_fingerprint(&watched_paths);
                failed = false;
                writeln!(messages, "rebuilt {}", plan.source.display()).map_err(|error| {
                    CliError::new(format!("failed to write rebuild status: {error}"))
                })?;
            }
            Err(error) => {
                write_error(messages, &error.to_string())?;
                watched_paths = configured_watch_paths(&plan);
                fingerprint = paths_fingerprint(&watched_paths);
                failed = true;
            }
        }
    }
}

fn configured_watch_paths(plan: &BuildPlan) -> Vec<PathBuf> {
    let mut paths = vec![plan.source.clone()];
    if let Some(config) = &plan.watched_config {
        paths.push(config.clone());
    }
    if let Some(template) = &plan.package.html_template {
        paths.push(template.clone());
    }
    if let Some(public) = &plan.package.public_dir {
        paths.push(public.clone());
    }
    paths
}

fn open_browser(url: &str) -> Result<(), CliError> {
    #[cfg(target_os = "macos")]
    let child = Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let child = Command::new("cmd").args(["/C", "start", "", url]).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let child = Command::new("xdg-open").arg(url).spawn();
    child
        .map(|_| ())
        .map_err(|error| CliError::new(format!("failed to open development server: {error}")))
}

fn accept_pending(listener: &TcpListener, state: &Arc<ServerState>) -> Result<(), CliError> {
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let Some(permit) = ConnectionPermit::acquire(Arc::clone(state)) else {
                    let _ = respond(
                        &mut stream,
                        503,
                        "text/plain; charset=utf-8",
                        b"too many connections",
                    );
                    continue;
                };
                let state = Arc::clone(state);
                thread::spawn(move || {
                    let _permit = permit;
                    if let Err(error) = handle_connection(stream, &state) {
                        eprintln!("development server connection failed: {error}");
                    }
                });
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) => {
                return Err(CliError::new(format!(
                    "development server accept failed: {error}"
                )));
            }
        }
    }
}

struct ConnectionPermit {
    state: Arc<ServerState>,
}

impl ConnectionPermit {
    fn acquire(state: Arc<ServerState>) -> Option<Self> {
        state
            .active_connections
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
                (active < MAX_HTTP_CONNECTIONS).then_some(active + 1)
            })
            .ok()?;
        Some(Self { state })
    }
}

impl Drop for ConnectionPermit {
    fn drop(&mut self) {
        self.state.active_connections.fetch_sub(1, Ordering::AcqRel);
    }
}

fn handle_connection(mut stream: TcpStream, state: &Arc<ServerState>) -> io::Result<()> {
    stream.set_read_timeout(Some(HTTP_READ_TIMEOUT))?;
    let request = read_request(&mut stream)?;
    let Some((method, target)) = request_line(&request) else {
        return respond(
            &mut stream,
            400,
            "text/plain; charset=utf-8",
            b"bad request",
        );
    };
    if !matches!(method, "GET" | "HEAD") {
        return respond(
            &mut stream,
            405,
            "text/plain; charset=utf-8",
            b"method not allowed",
        );
    }
    let head = method == "HEAD";
    if !valid_http_host(&request, state.port) {
        return respond_for_method(
            &mut stream,
            403,
            "text/plain; charset=utf-8",
            b"invalid host",
            head,
        );
    }
    if target == "/__polygl_ws" {
        if method != "GET" {
            return respond_for_method(
                &mut stream,
                405,
                "text/plain; charset=utf-8",
                b"method not allowed",
                head,
            );
        }
        return upgrade_websocket(stream, &request, state);
    }
    serve_static(
        &mut stream,
        &state.active,
        &state.error,
        target,
        head,
        request_header(&request, "if-none-match"),
    )
}

fn request_header<'request>(request: &'request [u8], name: &str) -> Option<&'request str> {
    std::str::from_utf8(request)
        .ok()
        .and_then(|request| header_value(request, name))
}

fn read_request(stream: &mut impl Read) -> io::Result<Vec<u8>> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        if request.len() > MAX_REQUEST_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "HTTP request headers are too large",
            ));
        }
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    Ok(request)
}

fn request_line(request: &[u8]) -> Option<(&str, &str)> {
    let request = std::str::from_utf8(request).ok()?;
    let mut parts = request.lines().next()?.split_whitespace();
    let method = parts.next()?;
    let target = parts.next()?.split('?').next()?;
    Some((method, target))
}

fn upgrade_websocket(
    mut stream: TcpStream,
    request: &[u8],
    state: &Arc<ServerState>,
) -> io::Result<()> {
    let request = std::str::from_utf8(request)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "request is not UTF-8"))?;
    let key = header_value(request, "sec-websocket-key");
    let upgrade = header_value(request, "upgrade");
    let connection = header_value(request, "connection");
    let version = header_value(request, "sec-websocket-version");
    let valid = key.is_some_and(valid_websocket_key)
        && upgrade.is_some_and(|value| value.eq_ignore_ascii_case("websocket"))
        && connection.is_some_and(|value| {
            value
                .split(',')
                .any(|token| token.trim().eq_ignore_ascii_case("upgrade"))
        })
        && version == Some("13")
        && same_origin(request, state.port);
    if !valid {
        return respond(
            &mut stream,
            400,
            "text/plain; charset=utf-8",
            b"invalid WebSocket upgrade",
        );
    }
    let key = key.expect("valid upgrades have a key");
    let mut clients = state
        .clients
        .lock()
        .map_err(|_| io::Error::other("WebSocket client lock is poisoned"))?;
    if clients.len() >= MAX_WEBSOCKET_CLIENTS {
        return respond(
            &mut stream,
            503,
            "text/plain; charset=utf-8",
            b"too many WebSocket clients",
        );
    }

    let accept = websocket_accept(key);
    write!(
        stream,
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {accept}\r\n\r\n"
    )?;
    stream.set_read_timeout(None)?;
    let writer_stream = stream.try_clone()?;
    writer_stream.set_write_timeout(Some(CLIENT_WRITE_TIMEOUT))?;
    let writer = Arc::new(Mutex::new(writer_stream));
    let id = NEXT_CLIENT.fetch_add(1, Ordering::Relaxed);
    clients.push(WebSocketClient {
        id,
        writer: Arc::clone(&writer),
    });
    drop(clients);

    let clients = Arc::clone(&state.clients);
    thread::spawn(move || reap_client(stream, id, writer, &clients));
    Ok(())
}

fn same_origin(request: &str, port: u16) -> bool {
    let Some(host) = header_value(request, "host") else {
        return false;
    };
    let Some(origin) = header_value(request, "origin") else {
        return false;
    };
    host_is_loopback(host, port) && origin.eq_ignore_ascii_case(&format!("http://{host}"))
}

fn valid_http_host(request: &[u8], port: u16) -> bool {
    let Ok(request) = std::str::from_utf8(request) else {
        return false;
    };
    let mut hosts = request.lines().filter_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("host").then(|| value.trim())
    });
    let Some(host) = hosts.next() else {
        return false;
    };
    hosts.next().is_none() && host_is_loopback(host, port)
}

fn host_is_loopback(host: &str, port: u16) -> bool {
    host.eq_ignore_ascii_case(&format!("127.0.0.1:{port}"))
        || host.eq_ignore_ascii_case(&format!("localhost:{port}"))
        || (port == 80
            && (host.eq_ignore_ascii_case("127.0.0.1") || host.eq_ignore_ascii_case("localhost")))
}

fn valid_websocket_key(key: &str) -> bool {
    key.len() == 24
        && key.ends_with("==")
        && key[..22]
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/'))
}

fn header_value<'request>(request: &'request str, expected: &str) -> Option<&'request str> {
    request.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case(expected).then(|| value.trim())
    })
}

fn reap_client(mut reader: TcpStream, id: u64, writer: Arc<Mutex<TcpStream>>, clients: &Clients) {
    loop {
        match read_client_frame(&mut reader) {
            Ok(Some((0x8, payload))) => {
                let _ = write_frame(&writer, 0x8, &payload);
                break;
            }
            Ok(Some((0x9, payload))) => {
                if write_frame(&writer, 0xA, &payload).is_err() {
                    break;
                }
            }
            Ok(Some(_)) => {}
            Ok(None) => break,
            Err(_) => {
                let _ = write_frame(&writer, 0x8, &1002_u16.to_be_bytes());
                break;
            }
        }
    }
    remove_client(clients, id);
}

fn read_client_frame(stream: &mut impl Read) -> io::Result<Option<(u8, Vec<u8>)>> {
    let mut header = [0_u8; 2];
    match stream.read_exact(&mut header) {
        Ok(()) => {}
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::UnexpectedEof
                    | io::ErrorKind::ConnectionReset
                    | io::ErrorKind::ConnectionAborted
            ) =>
        {
            return Ok(None);
        }
        Err(error) => return Err(error),
    }
    let opcode = header[0] & 0x0f;
    let final_frame = header[0] & 0x80 != 0;
    let reserved_bits = header[0] & 0x70;
    let masked = header[1] & 0x80 != 0;
    if reserved_bits != 0 || !final_frame || !matches!(opcode, 0x1 | 0x2 | 0x8 | 0x9 | 0xA) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported WebSocket frame",
        ));
    }
    if !masked {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "client WebSocket frames must be masked",
        ));
    }
    let mut length = u64::from(header[1] & 0x7f);
    if length == 126 {
        let mut bytes = [0_u8; 2];
        stream.read_exact(&mut bytes)?;
        length = u64::from(u16::from_be_bytes(bytes));
    } else if length == 127 {
        let mut bytes = [0_u8; 8];
        stream.read_exact(&mut bytes)?;
        if bytes[0] & 0x80 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid WebSocket frame length",
            ));
        }
        length = u64::from_be_bytes(bytes);
    }
    if length > MAX_CLIENT_FRAME_BYTES as u64 || (opcode >= 0x8 && length > 125) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "WebSocket frame is too large",
        ));
    }
    let mut mask = [0_u8; 4];
    stream.read_exact(&mut mask)?;
    let mut payload = vec![0_u8; length as usize];
    stream.read_exact(&mut payload)?;
    for (index, byte) in payload.iter_mut().enumerate() {
        *byte ^= mask[index % mask.len()];
    }
    if opcode == 0x8 && payload.len() == 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid WebSocket close payload",
        ));
    }
    Ok(Some((opcode, payload)))
}

fn remove_client(clients: &Clients, id: u64) {
    if let Ok(mut clients) = clients.lock() {
        clients.retain(|client| client.id != id);
    }
}

fn serve_static(
    stream: &mut TcpStream,
    active: &ActiveGeneration,
    current_error: &CurrentError,
    target: &str,
    head: bool,
    if_none_match: Option<&str>,
) -> io::Result<()> {
    let Some(relative) = safe_relative_path(target) else {
        return respond_for_method(stream, 403, "text/plain; charset=utf-8", b"forbidden", head);
    };
    let generation = active
        .read()
        .map_err(|_| io::Error::other("generation lock is poisoned"))?
        .clone();
    let error = current_error
        .read()
        .map_err(|_| io::Error::other("build error lock is poisoned"))?
        .clone();

    if relative == Path::new("index.html") {
        let html = if let Some(generation) = &generation {
            fs::read_to_string(generation.root.join("index.html"))?
        } else {
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>PolyGL build error</title></head><body></body></html>".to_owned()
        };
        let html = decorate_index(html, error.as_deref());
        return respond_static(
            stream,
            "text/html; charset=utf-8",
            html.as_bytes(),
            head,
            if_none_match,
        );
    }

    let Some(generation) = generation else {
        return respond_for_method(stream, 404, "text/plain; charset=utf-8", b"not found", head);
    };
    let requested = generation.root.join(relative);
    let requested = if requested.is_dir() {
        requested.join("index.html")
    } else {
        requested
    };
    let canonical_root = generation.root.canonicalize()?;
    let canonical = match requested.canonicalize() {
        Ok(path) if path.starts_with(&canonical_root) => path,
        _ => {
            return respond_for_method(
                stream,
                404,
                "text/plain; charset=utf-8",
                b"not found",
                head,
            );
        }
    };
    let contents = fs::read(&canonical)?;
    respond_static(
        stream,
        content_type(&canonical),
        &contents,
        head,
        if_none_match,
    )
}

fn safe_relative_path(target: &str) -> Option<PathBuf> {
    let target = target.strip_prefix('/')?;
    let decoded = percent_decode(target)?;
    if decoded.contains('\\') {
        return None;
    }
    let target = if decoded.is_empty() {
        "index.html"
    } else {
        decoded.as_str()
    };
    let path = Path::new(target);
    if path
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        Some(path.to_path_buf())
    } else {
        None
    }
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            decoded.push(bytes[index]);
            index += 1;
            continue;
        }
        let high = hex_value(*bytes.get(index + 1)?)?;
        let low = hex_value(*bytes.get(index + 2)?)?;
        decoded.push((high << 4) | low);
        index += 3;
    }
    let decoded = String::from_utf8(decoded).ok()?;
    (!decoded.chars().any(char::is_control)).then_some(decoded)
}

const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn respond(stream: &mut TcpStream, status: u16, content_type: &str, body: &[u8]) -> io::Result<()> {
    respond_for_method(stream, status, content_type, body, false)
}

fn respond_for_method(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
    head: bool,
) -> io::Result<()> {
    let reason = match status {
        200 => "OK",
        304 => "Not Modified",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        503 => "Service Unavailable",
        _ => "Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Length: {}\r\n\
         Content-Type: {content_type}\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n\r\n",
        body.len()
    )?;
    if !head {
        stream.write_all(body)?;
    }
    Ok(())
}

fn respond_static(
    stream: &mut TcpStream,
    content_type: &str,
    body: &[u8],
    head: bool,
    if_none_match: Option<&str>,
) -> io::Result<()> {
    let etag = static_etag(body);
    let not_modified = if_none_match.is_some_and(|value| etag_matches(value, &etag));
    let (status, reason, length) = if not_modified {
        (304, "Not Modified", 0)
    } else {
        (200, "OK", body.len())
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Length: {length}\r\n\
         Content-Type: {content_type}\r\n\
         ETag: {etag}\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n\r\n",
    )?;
    if !head && !not_modified {
        stream.write_all(body)?;
    }
    Ok(())
}

fn static_etag(body: &[u8]) -> String {
    format!("\"{}\"", blake3::hash(body).to_hex())
}

fn etag_matches(value: &str, current: &str) -> bool {
    value.split(',').any(|candidate| {
        let candidate = candidate.trim();
        candidate == "*" || candidate.strip_prefix("W/").unwrap_or(candidate) == current
    })
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("map" | "json") => "application/json",
        Some("css") => "text/css; charset=utf-8",
        Some("txt") => "text/plain; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("wasm") => "application/wasm",
        Some("mp3") => "audio/mpeg",
        Some("ogg") => "audio/ogg",
        Some("wav") => "audio/wav",
        _ => "application/octet-stream",
    }
}

fn decorate_index(mut html: String, error: Option<&str>) -> String {
    remove_marked(&mut html, CLIENT_START, CLIENT_END);
    remove_marked(&mut html, ERROR_START, ERROR_END);
    let mut injected = format!("{CLIENT_START}{DEV_CLIENT}{CLIENT_END}");
    if let Some(error) = error {
        injected.push_str(ERROR_START);
        injected.push_str(&format!(
            "<pre id=\"{INITIAL_ERROR_ID}\" hidden>{}</pre>",
            html_escape(error)
        ));
        injected.push_str(ERROR_END);
    }
    if let Some(position) = html.rfind("</body>") {
        html.insert_str(position, &injected);
    } else {
        html.push_str(&injected);
    }
    html
}

fn remove_marked(html: &mut String, start_marker: &str, end_marker: &str) {
    while let Some(start) = html.find(start_marker) {
        let Some(relative_end) = html[start..].find(end_marker) else {
            html.truncate(start);
            return;
        };
        let end = start + relative_end + end_marker.len();
        html.replace_range(start..end, "");
    }
}

fn html_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            other => escaped.push(other),
        }
    }
    escaped
}

fn write_error(messages: &mut dyn Write, message: &str) -> Result<(), CliError> {
    writeln!(messages, "{message}")
        .map_err(|error| CliError::new(format!("failed to write build error: {error}")))
}

fn start_broadcast_worker(clients: Clients) -> Result<BroadcastSender, CliError> {
    let (sender, receiver) = mpsc::channel::<String>();
    thread::Builder::new()
        .name("polygl-broadcast".to_owned())
        .spawn(move || {
            while let Ok(message) = receiver.recv() {
                broadcast_now(&clients, &message);
            }
        })
        .map_err(|error| CliError::new(format!("failed to start broadcast worker: {error}")))?;
    Ok(sender)
}

fn broadcast(sender: &BroadcastSender, message: &str) {
    let _ = sender.send(message.to_owned());
}

fn broadcast_now(clients: &Clients, message: &str) {
    let snapshot = match clients.lock() {
        Ok(clients) => clients.clone(),
        Err(_) => return,
    };
    let mut failed = Vec::new();
    for client in snapshot {
        if write_frame(&client.writer, 0x1, message.as_bytes()).is_err() {
            failed.push(client.id);
        }
    }
    if !failed.is_empty()
        && let Ok(mut clients) = clients.lock()
    {
        clients.retain(|client| !failed.contains(&client.id));
    }
}

fn write_frame(writer: &Mutex<TcpStream>, opcode: u8, payload: &[u8]) -> io::Result<()> {
    let frame = websocket_frame(opcode, payload);
    writer
        .lock()
        .map_err(|_| io::Error::other("WebSocket writer lock is poisoned"))?
        .write_all(&frame)
}

fn websocket_frame(opcode: u8, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(payload.len() + 10);
    frame.push(0x80 | opcode);
    match payload.len() {
        length @ 0..=125 => frame.push(length as u8),
        length @ 126..=65_535 => {
            frame.push(126);
            frame.extend_from_slice(&(length as u16).to_be_bytes());
        }
        length => {
            frame.push(127);
            frame.extend_from_slice(&(length as u64).to_be_bytes());
        }
    }
    frame.extend_from_slice(payload);
    frame
}

fn websocket_accept(key: &str) -> String {
    let mut input = Vec::with_capacity(key.len() + WEBSOCKET_GUID.len());
    input.extend_from_slice(key.as_bytes());
    input.extend_from_slice(WEBSOCKET_GUID.as_bytes());
    base64(&sha1(&input))
}

fn path_fingerprint(path: &Path) -> Option<(SystemTime, u64, u64)> {
    let metadata = fs::metadata(path).ok()?;
    let mut hasher = DefaultHasher::new();
    fs::read(path).ok()?.hash(&mut hasher);
    Some((metadata.modified().ok()?, metadata.len(), hasher.finish()))
}

fn paths_fingerprint(paths: &[PathBuf]) -> Option<u64> {
    let mut hasher = DefaultHasher::new();
    for path in paths {
        path.hash(&mut hasher);
        fingerprint_tree(path, &mut hasher).ok()?;
    }
    Some(hasher.finish())
}

fn fingerprint_tree(path: &Path, hasher: &mut DefaultHasher) -> io::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            0_u8.hash(hasher);
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    if metadata.file_type().is_symlink() {
        1_u8.hash(hasher);
        fs::read_link(path)?.hash(hasher);
    } else if metadata.is_file() {
        2_u8.hash(hasher);
        metadata.len().hash(hasher);
        fs::read(path)?.hash(hasher);
    } else if metadata.is_dir() {
        3_u8.hash(hasher);
        let mut entries = fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            entry.file_name().hash(hasher);
            fingerprint_tree(&entry.path(), hasher)?;
        }
    } else {
        4_u8.hash(hasher);
    }
    Ok(())
}

fn current_watch_paths(state: &ServerState, source: &Path) -> Result<Vec<PathBuf>, CliError> {
    let generation = state
        .active
        .read()
        .map_err(|_| CliError::new("generation lock is poisoned"))?;
    Ok(generation.as_ref().map_or_else(
        || vec![source.to_path_buf()],
        |value| value.watched_paths.clone(),
    ))
}

fn sha1(input: &[u8]) -> [u8; 20] {
    let mut message = input.to_vec();
    let bit_length = (message.len() as u64).wrapping_mul(8);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_length.to_be_bytes());

    let mut state = [
        0x6745_2301_u32,
        0xefcd_ab89,
        0x98ba_dcfe,
        0x1032_5476,
        0xc3d2_e1f0,
    ];
    for chunk in message.chunks_exact(64) {
        let mut words = [0_u32; 80];
        for (index, word) in chunk.chunks_exact(4).enumerate() {
            words[index] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for index in 16..80 {
            words[index] =
                (words[index - 3] ^ words[index - 8] ^ words[index - 14] ^ words[index - 16])
                    .rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = state;
        for (index, word) in words.iter().enumerate() {
            let (function, constant) = match index {
                0..=19 => ((b & c) | ((!b) & d), 0x5a82_7999),
                20..=39 => (b ^ c ^ d, 0x6ed9_eba1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1b_bcdc),
                _ => (b ^ c ^ d, 0xca62_c1d6),
            };
            let temporary = a
                .rotate_left(5)
                .wrapping_add(function)
                .wrapping_add(e)
                .wrapping_add(constant)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temporary;
        }
        for (slot, value) in state.iter_mut().zip([a, b, c, d, e]) {
            *slot = slot.wrapping_add(value);
        }
    }

    let mut digest = [0_u8; 20];
    for (chunk, value) in digest.chunks_exact_mut(4).zip(state) {
        chunk.copy_from_slice(&value.to_be_bytes());
    }
    digest
}

fn base64(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        output.push(ALPHABET[(first >> 2) as usize] as char);
        output.push(ALPHABET[(((first & 0x03) << 4) | (second >> 4)) as usize] as char);
        if chunk.len() > 1 {
            output.push(ALPHABET[(((second & 0x0f) << 2) | (third >> 6)) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(ALPHABET[(third & 0x3f) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{Cursor, Read, Write};
    use std::net::{Shutdown, TcpListener, TcpStream};
    use std::path::Path;
    use std::sync::atomic::AtomicUsize;
    use std::sync::{Arc, Mutex, RwLock};
    use std::thread;
    use std::time::{Duration, Instant};

    use super::{
        CLIENT_START, ConnectionPermit, DEV_CLIENT, ERROR_START, Generation, MAX_HTTP_CONNECTIONS,
        MAX_REQUEST_BYTES, ServerState, broadcast, content_type, decorate_index, etag_matches,
        handle_connection, paths_fingerprint, read_client_frame, read_request, safe_relative_path,
        same_origin, serve, start_broadcast_worker, static_etag, valid_http_host,
        valid_websocket_key, websocket_accept, websocket_frame,
    };

    #[test]
    fn implements_the_rfc_websocket_handshake() {
        assert_eq!(
            websocket_accept("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
        assert!(valid_websocket_key("dGhlIHNhbXBsZSBub25jZQ=="));
        assert!(!valid_websocket_key("not-a-nonce"));
    }

    #[test]
    fn accepts_only_the_development_server_origin() {
        let request = "Host: 127.0.0.1:4173\r\nOrigin: http://127.0.0.1:4173\r\n";
        assert!(same_origin(request, 4173));
        let foreign = "Host: 127.0.0.1:4173\r\nOrigin: https://example.com\r\n";
        assert!(!same_origin(foreign, 4173));
        assert!(valid_http_host(
            b"GET / HTTP/1.1\r\nHost: localhost:4173\r\n\r\n",
            4173,
        ));
        assert!(!valid_http_host(
            b"GET / HTTP/1.1\r\nHost: example.test\r\n\r\n",
            4173,
        ));
        assert!(!valid_http_host(
            b"GET / HTTP/1.1\r\nHost: localhost:4173\r\nHost: 127.0.0.1:4173\r\n\r\n",
            4173,
        ));
    }

    #[test]
    fn rejects_paths_that_can_escape_the_serve_root() {
        assert_eq!(
            safe_relative_path("/assets/app.js").unwrap(),
            std::path::PathBuf::from("assets/app.js")
        );
        assert_eq!(
            safe_relative_path("/my%20image.png").unwrap(),
            std::path::PathBuf::from("my image.png")
        );
        assert!(safe_relative_path("/../secret").is_none());
        assert!(safe_relative_path("/%2e%2e/secret").is_none());
        assert!(safe_relative_path("/nested%5csecret").is_none());
        assert!(safe_relative_path("/bad%2").is_none());
        assert!(safe_relative_path("/nested\\secret").is_none());
    }

    #[test]
    fn checks_header_size_before_accepting_the_terminator() {
        let mut oversized = vec![b'a'; MAX_REQUEST_BYTES + 4];
        let end = oversized.len();
        oversized[end - 4..].copy_from_slice(b"\r\n\r\n");
        let error = read_request(&mut Cursor::new(oversized)).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn matches_strong_weak_and_list_etags_without_accepting_stale_values() {
        let current = static_etag(b"current generation");
        assert!(etag_matches(&current, &current));
        assert!(etag_matches(&format!("W/{current}"), &current));
        assert!(etag_matches(&format!("\"stale\", {current}"), &current));
        assert!(etag_matches("*", &current));
        assert!(!etag_matches("\"stale\"", &current));
    }

    #[test]
    fn serves_declared_web_asset_mime_types_and_bounded_reconnects() {
        assert_eq!(
            content_type(Path::new("style.css")),
            "text/css; charset=utf-8"
        );
        assert_eq!(content_type(Path::new("icon.svg")), "image/svg+xml");
        assert_eq!(content_type(Path::new("module.wasm")), "application/wasm");
        assert!(DEV_CLIENT.contains("Math.min(10_000"));
        assert!(DEV_CLIENT.contains("Math.random()"));
        assert!(DEV_CLIENT.contains("retry = 0"));
    }

    #[test]
    fn rejects_unsupported_websocket_frame_flags_and_opcodes() {
        for frame in [
            vec![0x01, 0x80, 0, 0, 0, 0],
            vec![0xC1, 0x80, 0, 0, 0, 0],
            vec![0x80, 0x80, 0, 0, 0, 0],
            vec![0x81, 0x00],
        ] {
            assert!(read_client_frame(&mut Cursor::new(frame)).is_err());
        }
        assert_eq!(
            read_client_frame(&mut Cursor::new(vec![0x89, 0x80, 0, 0, 0, 0])).unwrap(),
            Some((0x9, Vec::new()))
        );
    }

    #[test]
    fn frames_short_and_extended_websocket_messages() {
        assert_eq!(websocket_frame(0x1, b"ok"), [0x81, 2, b'o', b'k']);
        let message = "x".repeat(126);
        let frame = websocket_frame(0x1, message.as_bytes());
        assert_eq!(&frame[..4], &[0x81, 126, 0, 126]);
        assert_eq!(&frame[4..], message.as_bytes());
    }

    #[test]
    fn bounds_concurrent_http_connections() {
        let clients = Arc::new(Mutex::new(Vec::new()));
        let broadcaster = start_broadcast_worker(Arc::clone(&clients)).unwrap();
        let state = Arc::new(ServerState {
            active: Arc::new(RwLock::new(None)),
            error: Arc::new(RwLock::new(None)),
            clients,
            broadcaster,
            active_connections: AtomicUsize::new(0),
            port: 4173,
        });
        let mut permits = (0..MAX_HTTP_CONNECTIONS)
            .map(|_| ConnectionPermit::acquire(Arc::clone(&state)).unwrap())
            .collect::<Vec<_>>();
        assert!(ConnectionPermit::acquire(Arc::clone(&state)).is_none());
        permits.pop();
        assert!(ConnectionPermit::acquire(state).is_some());
    }

    #[test]
    fn decorates_once_and_escapes_diagnostics_as_text() {
        let first = decorate_index(
            "<!doctype html><html><body><p>last good build</p></body></html>".to_owned(),
            Some("</script><img src=x onerror=alert(1)>"),
        );
        let second = decorate_index(first, Some("second error"));
        assert_eq!(second.matches(CLIENT_START).count(), 1);
        assert_eq!(second.matches(ERROR_START).count(), 1);
        assert!(second.contains("last good build"));
        assert!(second.contains("second error"));
        assert!(!second.contains("<img src=x"));
        assert!(!second.contains("</script><img"));
    }

    #[test]
    #[ignore = "requires loopback sockets"]
    fn serves_a_decorated_generation_over_a_real_tcp_connection() {
        let root = temporary_path("http");
        fs::create_dir(&root).unwrap();
        fs::write(
            root.join("index.html"),
            "<!doctype html><html><body>ready</body></html>",
        )
        .unwrap();
        let clients = Arc::new(Mutex::new(Vec::new()));
        let broadcaster = start_broadcast_worker(Arc::clone(&clients)).unwrap();
        let state = Arc::new(ServerState {
            active: Arc::new(RwLock::new(Some(Arc::new(Generation {
                root,
                watched_paths: Vec::new(),
            })))),
            error: Arc::new(RwLock::new(None)),
            clients,
            broadcaster,
            active_connections: AtomicUsize::new(0),
            port: 4173,
        });
        let response = round_trip(&state, "GET / HTTP/1.1\r\nHost: 127.0.0.1:4173\r\n\r\n");
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("Cache-Control: no-store"));
        let etag = response
            .lines()
            .find_map(|line| line.strip_prefix("ETag: "))
            .unwrap()
            .to_owned();
        assert!(response.contains(CLIENT_START));
        assert!(response.contains("ready"));

        let head = round_trip(&state, "HEAD / HTTP/1.1\r\nHost: 127.0.0.1:4173\r\n\r\n");
        assert!(head.starts_with("HTTP/1.1 200 OK"));
        assert!(head.contains(&format!("ETag: {etag}")));
        assert!(!head.contains("ready"));

        let cached = round_trip(
            &state,
            format!("GET / HTTP/1.1\r\nHost: 127.0.0.1:4173\r\nIf-None-Match: {etag}\r\n\r\n"),
        );
        assert!(cached.starts_with("HTTP/1.1 304 Not Modified"));
        assert!(!cached.contains("ready"));

        let rebound = round_trip(&state, "GET / HTTP/1.1\r\nHost: example.test\r\n\r\n");
        assert!(rebound.starts_with("HTTP/1.1 403 Forbidden"));

        let rejected = round_trip(
            &state,
            "GET /__polygl_ws HTTP/1.1\r\n\
             Host: 127.0.0.1:4173\r\n\
             Origin: https://example.com\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Version: 13\r\n\
             Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
        );
        assert!(rejected.starts_with("HTTP/1.1 400 Bad Request"));
    }

    #[test]
    #[ignore = "requires loopback sockets"]
    fn broadcasts_reload_to_an_upgraded_websocket() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let clients = Arc::new(Mutex::new(Vec::new()));
        let broadcaster = start_broadcast_worker(Arc::clone(&clients)).unwrap();
        let state = Arc::new(ServerState {
            active: Arc::new(RwLock::new(None)),
            error: Arc::new(RwLock::new(None)),
            clients,
            broadcaster,
            active_connections: AtomicUsize::new(0),
            port: address.port(),
        });
        let server_state = Arc::clone(&state);
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            handle_connection(stream, &server_state).unwrap();
        });

        let mut client = TcpStream::connect(address).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        write!(
            client,
            "GET /__polygl_ws HTTP/1.1\r\n\
             Host: 127.0.0.1:{port}\r\n\
             Origin: http://127.0.0.1:{port}\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Version: 13\r\n\
             Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
            port = address.port()
        )
        .unwrap();
        let mut response = Vec::new();
        let mut buffer = [0_u8; 256];
        while !response.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = client.read(&mut buffer).unwrap();
            assert_ne!(read, 0, "WebSocket handshake ended before the headers");
            response.extend_from_slice(&buffer[..read]);
        }
        assert!(String::from_utf8_lossy(&response).starts_with("HTTP/1.1 101 Switching Protocols"));
        server.join().unwrap();

        let deadline = Instant::now() + Duration::from_secs(2);
        while state.clients.lock().unwrap().is_empty() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(state.clients.lock().unwrap().len(), 1);
        broadcast(&state.broadcaster, r#"{"type":"reload"}"#);

        let expected = websocket_frame(0x1, br#"{"type":"reload"}"#);
        let mut actual = vec![0_u8; expected.len()];
        client.read_exact(&mut actual).unwrap();
        assert_eq!(actual, expected);
        client.shutdown(Shutdown::Both).unwrap();
    }

    #[test]
    fn non_watch_mode_returns_the_initial_compile_error() {
        let root = temporary_path("invalid");
        fs::create_dir(&root).unwrap();
        let source = root.join("main.rb");
        fs::write(&source, "def setup\n").unwrap();
        let error = serve(&source, false, 0, false, &mut Vec::new()).unwrap_err();
        assert!(error.to_string().contains("E0100"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fingerprints_missing_paths_and_recursive_directory_changes() {
        let root = temporary_path("fingerprint-tree");
        fs::create_dir(&root).unwrap();
        let missing = root.join("polygl.toml");
        let watched = vec![root.clone(), missing.clone()];
        let initial = paths_fingerprint(&watched).unwrap();
        fs::write(root.join("source.rb"), "def setup\nend\n").unwrap();
        let with_source = paths_fingerprint(&watched).unwrap();
        assert_ne!(initial, with_source);
        fs::write(&missing, "entry = \"source.rb\"\n").unwrap();
        assert_ne!(with_source, paths_fingerprint(&watched).unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    fn round_trip(state: &Arc<ServerState>, request: impl Into<String>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let request = request.into();
        let client = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            stream.write_all(request.as_bytes()).unwrap();
            stream.shutdown(Shutdown::Write).unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();
            response
        });
        let (stream, _) = listener.accept().unwrap();
        handle_connection(stream, state).unwrap();
        client.join().unwrap()
    }

    fn temporary_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("polygl-serve-test-{label}-{}", std::process::id()))
    }
}
