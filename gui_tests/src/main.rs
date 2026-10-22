//! This is the entry point for the test executables.
//!
//! Run by `cargo run --bin gui_tests`

use std::process::{Command, ExitStatus};

fn run_test(name: &str) -> bool {
    println!("Running {}", name);
    let output = Command::new("cargo")
        .env("RUST_BACKTRACE", "1")
        .args(&["run", "--bin", name])
        .output()
        .unwrap();
    if output.status.success() {
        println!("Success");
        true
    } else {
        match output.status.code() {
            Some(exit_code) => println!("FAILED, exit code was {}", exit_code),
            None => println!("FAILED, terminated by signal."),
        }
        println!("Stdout from {}:", name);
        println!("{}", String::from_utf8_lossy(&output.stdout));
        println!();
        println!("Stderr from {}:", name);
        println!("{}", String::from_utf8_lossy(&output.stderr));
        false
    }
}

fn main() {
    let tests = ["basic_mouse", "resize"];
    let mut failed = 0;
    println!("Running {} tests", tests.len());
    for test_name in &tests {
        if !run_test(test_name) {
            failed += 1;
        }
    }
    println!(
        "Finished running {} tests. {} failed, {} succeeded.",
        tests.len(),
        failed,
        tests.len() - failed
    );
    if failed > 0 {
        std::process::exit(1);
    }
}
