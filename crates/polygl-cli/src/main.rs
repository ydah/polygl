use std::env;
use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    let color_supported = io::stdout().is_terminal();
    match polygl_cli::run_with_color_support(env::args_os().skip(1), &mut output, color_supported) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = output.flush();
            eprintln!("error: {error}");
            ExitCode::from(error.exit_code())
        }
    }
}
