use std::env;
use std::io::{self, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    match polygl_cli::run(env::args_os().skip(1), &mut output) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = output.flush();
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
