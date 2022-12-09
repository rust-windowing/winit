use cfg_aliases::cfg_aliases;

fn main() {
    // Setup cfg aliases
    cfg_aliases! {
        // Platforms
        linux: {
            any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
            )
        },
        macos: { target_os = "macos" },
        android: { target_os = "android" },
        ios: { target_os = "ios" },
        wasm: { target_arch = "wasm32" },
        x11: { all(linux, feature = "x11") },
        wayland: { all(linux, feature = "wayland") },

        // dependencies
        mint : { feature = "mint" },
        serde : { feature = "serde" },
        sctk_adwaita : { feature = "sctk-adwaita" },
    }
}
