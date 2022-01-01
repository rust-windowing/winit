use pico_args::Arguments;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

const HELP: &str = "\
cargo run-wasm-example

USAGE:
  cargo run-wasm-example [FLAGS] EXAMPLE

FLAGS:
  --release             Compile and run with the release profile

EXAMPLE:
  Name of the example in examples/ to run
";

macro_rules! help_and_return {
    ($($tts:tt)*) => {
        println!($($tts)*);
        println!("\n{}", HELP);
        return;
    }
}

fn main() {
    let mut args = Arguments::from_env();

    let release = args.contains("--release");
    let profile = if release { "release" } else { "debug" };

    let unused_args: Vec<String> = args
        .finish()
        .into_iter()
        .map(|x| x.into_string().unwrap())
        .collect();
    for unused_arg in &unused_args {
        if unused_arg.starts_with('-') {
            help_and_return!("Unknown option {}", unused_arg);
        }
    }
    if unused_args.len() != 1 {
        help_and_return!(
            "Expected exactly one free arg, but there was {} free args: {:?}",
            unused_args.len(),
            unused_args
        );
    }
    let example: String = unused_args[0].clone();
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());

    let cargo_args = [
        "build",
        "--target",
        "wasm32-unknown-unknown",
        // It is common to setup a faster linker such as mold or lld to run for just your native target.
        // It cant be set for wasm as wasm doesnt support building with these linkers.
        // This results in a separate rustflags value for native and wasm builds.
        // Currently rust triggers a full rebuild every time the rustflags value changes.
        //
        // Therefore we have this hack where we use a different target dir for wasm builds to avoid constantly triggering full rebuilds.
        // When this issue is resolved we might be able to remove this hack: https://github.com/rust-lang/cargo/issues/8716
        "--target-dir",
        "target/wasm-examples-target",
        "--example",
        &example,
        "--release",
    ];
    if let Err(err) = run_command(
        &cargo,
        // --release is the last arg so we strip it off when we are not using the release profile
        &if release {
            &cargo_args
        } else {
            &cargo_args[..cargo_args.len() - 1]
        },
    ) {
        println!("{}", err);
        return;
    }

    let wasm_source = Path::new("target/wasm-examples-target/wasm32-unknown-unknown")
        .join(profile)
        .join("examples")
        .join(format!("{}.wasm", &example));
    let example_dest = project_root().join("target/wasm-examples").join(&example);
    std::fs::create_dir_all(&example_dest).unwrap();
    if let Err(err) = run_command(
        "wasm-bindgen",
        &[
            "--target",
            "web",
            "--out-dir",
            example_dest.as_os_str().to_str().unwrap(),
            wasm_source.as_os_str().to_str().unwrap(),
        ],
    ) {
        println!("{}", err);
        return;
    }

    let index_template = include_str!("index.template.html");
    let index_processed = index_template.replace("{{example}}", &example);
    std::fs::write(example_dest.join("index.html"), index_processed).unwrap();
    println!("\nServing `{}` on http://localhost:8000", example);
    devserver_lib::run(
        "localhost",
        8000,
        example_dest.as_os_str().to_str().unwrap(),
        false,
        "",
    );
}

fn run_command(command: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(command)
        .current_dir(project_root())
        .args(args)
        .status()
        .unwrap();

    if !status.success() {
        Err("cargo build failed".to_string())
    } else {
        Ok(())
    }
}

fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(1)
        .unwrap()
        .to_path_buf()
}
