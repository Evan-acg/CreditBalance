mod application;
mod bootstrap;
mod domain;
mod infrastructure;
mod interfaces;

use std::process::ExitCode;

use interfaces::cli::Action;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let application = match bootstrap::build_application() {
        Ok(application) => application,
        Err(error) => {
            eprintln!("错误: {error:#}");
            return ExitCode::FAILURE;
        }
    };
    let providers = application.balance_provider_descriptors();
    match interfaces::cli::parse_args(&args, &providers) {
        Ok(Action::Help(usage)) => {
            println!("{usage}");
            ExitCode::SUCCESS
        }
        Ok(Action::Version) => {
            println!("crd {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Ok(Action::Run(command)) => match interfaces::cli::execute(command, &application) {
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
