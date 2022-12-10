use cfg_aliases::cfg_aliases;

fn main() {
    // Setup cfg aliases
    cfg_aliases! {
        // Platforms
        os_linuxy: {
            any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
            )
        },
        os_windows: { target_os = "windows" },
        os_macos: { target_os = "macos" },
        os_android: { target_os = "android" },
        os_ios: { target_os = "ios" },
        arch_wasm: { target_arch = "wasm32" },
        x11: { feature = "x11" },
        wayland: { feature = "wayland" },

        // dependencies
        mint : { feature = "mint" },
        serde : { feature = "serde" },
        sctk_adwaita : { feature = "sctk-adwaita" },
    }
}
