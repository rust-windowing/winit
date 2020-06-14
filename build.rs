#[cfg(all(
    any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ),
    not(feature = "x11"),
    not(feature = "wayland")
))]
compile_error!("at least one of the \"x11\"/\"wayland\" features must be enabled");

#[cfg(all(
    target_arch = "wasm32",
    not(feature = "web-sys"),
    not(feature = "stdweb")
))]
compile_error!("at least one of the \"web-sys\"/\"stdweb\" features must be enabled");

fn main() {}
