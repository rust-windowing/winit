use pico_args::Arguments;
use std::env;
use std::path::Path;
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

struct Args {
    release: bool,
    example: String,
}

impl Args {
    pub fn from_env() -> Result<Self, String> {
        let mut args = Arguments::from_env();
        let release = args.contains("--release");

        let mut unused_args: Vec<String> = args
            .finish()
            .into_iter()
            .map(|x| x.into_string().unwrap())
            .collect();

        for unused_arg in &unused_args {
            if unused_arg.starts_with('-') {
                return Err(format!("Unknown flag {}", unused_arg));
            }
        }

        match unused_args.len() {
            0 => Err("Expected EXAMPLE arg, but there was no EXAMPLE arg".to_string()),
            1 => Ok(Args {
                release,
                example: unused_args.remove(0),
            }),
            len => Err(format!(
                "Expected exactly one free arg, but there was {} free args: {:?}",
                len, unused_args
            )),
        }
    }
}

fn main() {
    let args = match Args::from_env() {
        Ok(args) => args,
        Err(err) => {
            println!("{}\n\n{}", err, HELP);
            return;
        }
    };
    let profile = if args.release { "release" } else { "debug" };

    // build wasm example via cargo
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let project_root = Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(1)
        .unwrap()
        .to_path_buf();
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
        &args.example,
        "--release",
    ];
    let status = Command::new(&cargo)
        .current_dir(&project_root)
        .args(
            // --release is the last arg so we strip it off when we are not using the release profile
            if args.release {
                &cargo_args
            } else {
                &cargo_args[..cargo_args.len() - 1]
            },
        )
        .status()
        .unwrap();
    if !status.success() {
        // We can return without printing anything because cargo will have already displayed an appropriate error.
        return;
    }

    // run wasm-bindgen on wasm file output by cargo, write to the destination folder
    let wasm_source = Path::new("target/wasm-examples-target/wasm32-unknown-unknown")
        .join(profile)
        .join("examples")
        .join(format!("{}.wasm", &args.example));
    let example_dest = project_root
        .join("target/wasm-examples")
        .join(&args.example);
    std::fs::create_dir_all(&example_dest).unwrap();
    let mut bindgen = wasm_bindgen_cli_support::Bindgen::new();
    bindgen
        .web(true)
        .unwrap()
        .omit_default_module_path(false)
        .input_path(&wasm_source)
        .generate(&example_dest)
        .unwrap();

    // process template index.html and write to the destination folder
    let index_template = include_str!("index.template.html");
    let index_processed = index_template.replace("{{example}}", &args.example);
    std::fs::write(example_dest.join("index.html"), index_processed).unwrap();

    // run webserver on destination folder
    println!("\nServing `{}` on http://localhost:8000", args.example);
    devserver_lib::run(
        "localhost",
        8000,
        example_dest.as_os_str().to_str().unwrap(),
        false,
        "",
    );
}
