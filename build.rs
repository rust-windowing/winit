fn main() {
    // If building for macos and WINIT_LINK_COLORSYNC is set to true
    // use CGDisplayCreateUUIDFromDisplayID from ColorSync instead of CoreGraphics
    if std::env::var("CARGO_CFG_TARGET_OS").map_or(false, |os| os == "macos")
        && std::env::var("WINIT_LINK_COLORSYNC")
            .map_or(false, |v| v == "1" || v.eq_ignore_ascii_case("true"))
    {
        println!("cargo:rustc-cfg=use_colorsync_cgdisplaycreateuuidfromdisplayid");
    }
}
