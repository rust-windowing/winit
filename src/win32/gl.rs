/// WGL bindings
pub mod wgl {
    generate_gl_bindings! {
        api: "wgl",
        profile: "core",
        version: "1.0",
        generator: "static"
    }
}

/// Functions that are not necessarly always available
pub mod wgl_extra {
    generate_gl_bindings! {
        api: "wgl",
        profile: "core",
        version: "1.0",
        generator: "struct",
        extensions: [
            "WGL_ARB_create_context",
            "WGL_EXT_swap_control"
        ]
    }
}

#[link(name = "opengl32")]
extern {}
