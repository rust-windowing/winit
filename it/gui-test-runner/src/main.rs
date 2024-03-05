//! Runner for the `gui-test` system.

use std::env;
use std::process::{Command, Stdio};

fn main() {
    let mut args = env::args();

    // Get the test crate name.
    let test_crate = args.nth(1).unwrap();

    // Get the target.
    let target_tag = args.next().unwrap();

    // Split the target into the target and the tag.
    let (target, tag) = {
        let mut split = target_tag.splitn(1, ':');
        let target = split.next().unwrap();
        let tag = split.next();
        (target, tag)
    };

    // Get the current target.
    let current_target = current_target();

    // For now, we only support building for the current target.
    assert_eq!(target, current_target);
    assert!(tag.is_none());

    // Just run the crate.
    if !Command::new("cargo")
        .args(["run", "-p", &test_crate])
        .status()
        .unwrap()
        .success()
    {
        panic!("test failed");
    }
}

/// Get the current target.
fn current_target() -> String {
    let output = Command::new("rustc")
        .arg("-vV")
        .stdout(Stdio::piped())
        .output()
        .unwrap();

    // Look for the line that starts with "host".
    let stdout = String::from_utf8(output.stdout).unwrap();
    for line in stdout.lines() {
        if let Some(host) = line.strip_prefix("host: ") {
            return host.to_string();
        }
    }

    panic!("failed to find host: line in rustc output")
}
