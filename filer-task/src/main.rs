use std::process::ExitCode;

use clap::Parser;
use filer_task::cli::{Cli, Command, run_list, run_validate};

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Validate(args) => run_validate(args),
        Command::List(args) => run_list(args),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
