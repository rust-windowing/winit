use cfg_aliases::cfg_aliases;

#[allow(
    clippy::disallowed_macros,
    reason = "Only relevant for examples and Winit, our usage of println! is fine here."
)]
#[allow(
    semicolon_in_expressions_from_macros,
    reason = "This is a future incompatibility lint and we currently use cfg_aliases 0.2.1, which \
              has not been updated to resolve the latest lints"
)]
fn main() {
    // Dummy invocation to enable change-tracking in build scripts.
    println!("cargo:rerun-if-changed=build.rs");

    // Setup cfg aliases.
    cfg_aliases! {
        // Systems.
        android_platform: { target_os = "android" },
        web_platform: { all(target_family = "wasm", target_os = "unknown") },
        macos_platform: { target_os = "macos" },
        ios_platform: { all(target_vendor = "apple", not(target_os = "macos")) },
        windows_platform: { target_os = "windows" },
        free_unix: { all(unix, not(target_vendor = "apple"), not(android_platform), not(target_os = "emscripten")) },
        redox: { target_os = "redox" },

        // Native displays.
        x11_platform: { all(feature = "x11", free_unix, not(redox)) },
        wayland_platform: { all(feature = "wayland", free_unix, not(redox)) },
        orbital_platform: { redox },
    }

    // Winit defined cfgs.
    println!("cargo:rustc-check-cfg=cfg(unreleased_changelogs)");
}
