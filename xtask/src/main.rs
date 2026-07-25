use std::env;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

mod generated;

use generated::generate_runtime;

const CONFORMANCE_LAYERS: [&str; 3] = ["l1-render", "l2-hir-snapshots", "l3-neutral-hir"];

fn main() -> ExitCode {
    match run(env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let Some(command) = args.next() else {
        return Err(usage());
    };

    match command.as_str() {
        "gen-runtime" => {
            let check = match args.next().as_deref() {
                None => false,
                Some("--check") => true,
                Some(_) => return Err(usage()),
            };
            ensure_no_more_args(args)?;
            generate_runtime(check).map_err(|error| error.to_string())
        }
        "conformance" => {
            ensure_no_more_args(args)?;
            check_conformance_layout().map_err(|error| error.to_string())
        }
        _ => Err(usage()),
    }
}

fn ensure_no_more_args(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    if args.next().is_some() {
        return Err(usage());
    }
    Ok(())
}

fn check_conformance_layout() -> io::Result<()> {
    let root = workspace_root().join("conformance");
    for layer in CONFORMANCE_LAYERS {
        let path = root.join(layer);
        if !path.is_dir() {
            return Err(io::Error::other(format!(
                "missing conformance layer: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must live directly below the workspace root")
        .to_path_buf()
}

fn usage() -> String {
    "usage: cargo xtask <gen-runtime [--check] | conformance>".to_owned()
}

#[cfg(test)]
mod tests {
    use super::{check_conformance_layout, generate_runtime};

    #[test]
    fn generated_runtime_is_current() {
        generate_runtime(true).expect("committed runtime operations must be current");
    }

    #[test]
    fn conformance_layers_are_present() {
        check_conformance_layout().expect("all conformance layers must be present");
    }
}
