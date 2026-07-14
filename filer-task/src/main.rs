use std::process::ExitCode;

use clap::Parser;
use filer_task::cli::{
    Cli, Command, run_add, run_block, run_context, run_criterion_toggle, run_defer, run_deps,
    run_done, run_import, run_init, run_list, run_milestone, run_obsolete, run_ready, run_show,
    run_start, run_summary, run_validate,
};

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Validate(args) => run_validate(args),
        Command::List(args) => run_list(args),
        Command::Show(args) => run_show(args),
        Command::Ready(args) => run_ready(args),
        Command::Context(args) => run_context(args),
        Command::Deps(args) => run_deps(args),
        Command::Milestone(args) => run_milestone(args),
        Command::Summary(args) => run_summary(args),
        Command::Init(args) => run_init(args),
        Command::Add(args) => run_add(*args),
        Command::Import(args) => run_import(args),
        Command::Start(args) => run_start(args),
        Command::Done(args) => run_done(args),
        Command::CriterionToggle(args) => run_criterion_toggle(args),
        Command::Block(args) => run_block(args),
        Command::Defer(args) => run_defer(args),
        Command::Obsolete(args) => run_obsolete(args),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
