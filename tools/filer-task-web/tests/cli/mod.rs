use std::process::{Command, Output};

pub fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_filer-task-web")
}

pub fn run(args: &[&str]) -> Output {
    Command::new(binary())
        .args(args)
        .output()
        .expect("CLI process spawns")
}
