use cfg_aliases::cfg_aliases;

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
            "wayland_protocols/fractional-scale-v1.xml",
            &path,
            Side::Client,
        );
    }
}

fn main() {
    // The script doesn't depend on our code
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=wayland_protocols");

    // Setup cfg aliases
    cfg_aliases! {
        // Systems.
        android_platform: { target_os = "android" },
        wasm_platform: { target_family = "wasm" },
        macos_platform: { target_os = "macos" },
        ios_platform: { target_os = "ios" },
        windows_platform: { target_os = "windows" },
        apple: { any(target_os = "ios", target_os = "macos") },
        free_unix: { all(unix, not(apple), not(android_platform)) },
        redox: { target_os = "redox" },

        // Native displays.
        x11_platform: { all(feature = "x11", free_unix, not(wasm), not(redox)) },
        wayland_platform: { all(feature = "wayland", free_unix, not(wasm), not(redox)) },
        orbital_platform: { redox },
    }

    // XXX aliases are not available for the build script itself.
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
