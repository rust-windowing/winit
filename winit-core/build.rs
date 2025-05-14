use cfg_aliases::cfg_aliases;

fn main() {
    // The script doesn't depend on our code.
    println!("cargo:rerun-if-changed=build.rs");

    // Setup cfg aliases.
    cfg_aliases! {
        web_platform: { all(target_family = "wasm", target_os = "unknown") },
    }
}
