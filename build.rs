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
compile_error!("Please select a feature to build for unix: `x11`, `wayland`");

#[cfg(all(
    target_arch = "wasm32",
    not(feature = "web-sys"),
    not(feature = "stdweb")
))]
compile_error!("Please select a feature to build for web: `web-sys`, `stdweb`");

fn main() {}
