mod application;
mod cache;
mod cli;
mod codex_usage;
mod output;
mod providers;

use std::process::ExitCode;

use cli::Action;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match cli::parse_args(&args) {
        Ok(Action::Help(usage)) => {
            println!("{usage}");
            ExitCode::SUCCESS
        }
        Ok(Action::Version) => {
            println!("crd {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Ok(Action::Run(command)) => match application::run(command) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("错误: {error:#}");
                ExitCode::FAILURE
            }
        },
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}
