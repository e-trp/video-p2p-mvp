use app_core::{AppCommand, parse_cli_args, print_help, run_receiver, run_sender};
use std::error::Error;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse_cli_args(&args)? {
        AppCommand::Help => {
            print_help();
            Ok(())
        }
        AppCommand::PrintSpec => {
            print!("{}", include_str!("../../../docs/SPECIFICATION.md"));
            Ok(())
        }
        AppCommand::Sender(config) => run_sender(config),
        AppCommand::Receiver(config) => run_receiver(config),
    }
}
