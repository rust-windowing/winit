#[cfg(all(
    any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
    ),
    feature = "wayland",
))]
mod wayland {
    use std::env;
    use std::path::PathBuf;
    use wayland_scanner::Side;

    pub fn main() {
        let mut path = PathBuf::from(env::var("OUT_DIR").unwrap());
        path.push("fractional_scale_v1.rs");
        wayland_scanner::generate_code(
            "build/protocols/fractional-scale-v1.xml",
            &path,
            Side::Client,
        );
    }
}

fn main() {
    #[cfg(all(
        any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd",
        ),
        feature = "wayland",
    ))]
    wayland::main();
}
