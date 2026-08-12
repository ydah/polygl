use std::env;
use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stdout = io::stdout();
    let mut output = stdout.lock();
    let color_supported = io::stdout().is_terminal();
    match polygl_cli::run_with_io(
        env::args_os().skip(1),
        &mut input,
        &mut output,
        color_supported,
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = output.flush();
            eprintln!("error: {error}");
            ExitCode::from(error.exit_code())
        }
    }
}
