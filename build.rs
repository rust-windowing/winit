extern crate gl_generator;
extern crate khronos_api;

use std::env;
use std::fs::File;
use std::path::PathBuf;

fn main() {
    let target = env::var("TARGET").unwrap();
    let dest = PathBuf::from(&env::var("OUT_DIR").unwrap());

    if target.contains("windows") {
        let mut file = File::create(&dest.join("wgl_bindings.rs")).unwrap();
        gl_generator::generate_bindings(gl_generator::StaticGenerator,
                                        gl_generator::registry::Ns::Wgl,
                                        gl_generator::Fallbacks::All,
                                        khronos_api::WGL_XML, vec![],
                                        "1.0", "core", &mut file).unwrap();

        let mut file = File::create(&dest.join("wgl_extra_bindings.rs")).unwrap();
        gl_generator::generate_bindings(gl_generator::StructGenerator,
                                        gl_generator::registry::Ns::Wgl,
                                        gl_generator::Fallbacks::All,
                                        khronos_api::WGL_XML,
                                        vec![
                                            "WGL_ARB_create_context".to_string(),
                                            "WGL_ARB_create_context_profile".to_string(),
                                            "WGL_ARB_create_context_robustness".to_string(),
                                            "WGL_ARB_extensions_string".to_string(),
                                            "WGL_ARB_framebuffer_sRGB".to_string(),
                                            "WGL_ARB_multisample".to_string(),
                                            "WGL_ARB_pixel_format".to_string(),
                                            "WGL_EXT_create_context_es2_profile".to_string(),
                                            "WGL_EXT_extensions_string".to_string(),
                                            "WGL_EXT_framebuffer_sRGB".to_string(),
                                            "WGL_EXT_swap_control".to_string(),
                                        ],
                                        "1.0", "core", &mut file).unwrap();

        let mut file = File::create(&dest.join("egl_bindings.rs")).unwrap();
        gl_generator::generate_bindings(gl_generator::StructGenerator,
                                        gl_generator::registry::Ns::Egl,
                                        gl_generator::Fallbacks::All,
                                        khronos_api::EGL_XML,
                                        vec![
                                            "EGL_KHR_create_context".to_string(),
                                            "EGL_EXT_create_context_robustness".to_string(),
                                            "EGL_KHR_create_context_no_error".to_string(),
                                            "EGL_KHR_platform_x11".to_string(),
                                            "EGL_KHR_platform_android".to_string(),
                                            "EGL_KHR_platform_wayland".to_string(),
                                            "EGL_KHR_platform_gbm".to_string(),
                                            "EGL_EXT_platform_base".to_string(),
                                            "EGL_EXT_platform_x11".to_string(),
                                            "EGL_MESA_platform_gbm".to_string(),
                                            "EGL_EXT_platform_wayland".to_string(),
                                            "EGL_EXT_platform_device".to_string(),
                                        ],
                                        "1.5", "core", &mut file).unwrap();
    }

    if target.contains("linux") || target.contains("dragonfly") || target.contains("freebsd") {
        let mut file = File::create(&dest.join("glx_bindings.rs")).unwrap();
        gl_generator::generate_bindings(gl_generator::StructGenerator,
                                        gl_generator::registry::Ns::Glx,
                                        gl_generator::Fallbacks::All,
                                        khronos_api::GLX_XML, vec![],
                                        "1.4", "core", &mut file).unwrap();

        let mut file = File::create(&dest.join("glx_extra_bindings.rs")).unwrap();
        gl_generator::generate_bindings(gl_generator::StructGenerator,
                                        gl_generator::registry::Ns::Glx,
                                        gl_generator::Fallbacks::All,
                                        khronos_api::GLX_XML,
                                        vec![
                                            "GLX_ARB_create_context".to_string(),
                                            "GLX_ARB_create_context_profile".to_string(),
                                            "GLX_ARB_create_context_robustness".to_string(),
                                            "GLX_ARB_framebuffer_sRGB".to_string(),
                                            "GLX_EXT_framebuffer_sRGB".to_string(),
                                            "GLX_EXT_swap_control".to_string(),
                                            "GLX_SGI_swap_control".to_string()
                                        ],
                                        "1.4", "core", &mut file).unwrap();

        let mut file = File::create(&dest.join("egl_bindings.rs")).unwrap();
        gl_generator::generate_bindings(gl_generator::StructGenerator,
                                        gl_generator::registry::Ns::Egl,
                                        gl_generator::Fallbacks::All,
                                        khronos_api::EGL_XML,
                                        vec![
                                            "EGL_KHR_create_context".to_string(),
                                            "EGL_EXT_create_context_robustness".to_string(),
                                            "EGL_KHR_create_context_no_error".to_string(),
                                            "EGL_KHR_platform_x11".to_string(),
                                            "EGL_KHR_platform_android".to_string(),
                                            "EGL_KHR_platform_wayland".to_string(),
                                            "EGL_KHR_platform_gbm".to_string(),
                                            "EGL_EXT_platform_base".to_string(),
                                            "EGL_EXT_platform_x11".to_string(),
                                            "EGL_MESA_platform_gbm".to_string(),
                                            "EGL_EXT_platform_wayland".to_string(),
                                            "EGL_EXT_platform_device".to_string(),
                                        ],
                                        "1.5", "core", &mut file).unwrap();
    }

    if target.contains("android") {
        let mut file = File::create(&dest.join("egl_bindings.rs")).unwrap();
        gl_generator::generate_bindings(gl_generator::StaticStructGenerator,
                                        gl_generator::registry::Ns::Egl,
                                        gl_generator::Fallbacks::All,
                                        khronos_api::EGL_XML,
                                        vec![
                                            "EGL_KHR_create_context".to_string(),
                                            "EGL_EXT_create_context_robustness".to_string(),
                                            "EGL_KHR_create_context_no_error".to_string(),
                                            "EGL_KHR_platform_x11".to_string(),
                                            "EGL_KHR_platform_android".to_string(),
                                            "EGL_KHR_platform_wayland".to_string(),
                                            "EGL_KHR_platform_gbm".to_string(),
                                            "EGL_EXT_platform_base".to_string(),
                                            "EGL_EXT_platform_x11".to_string(),
                                            "EGL_MESA_platform_gbm".to_string(),
                                            "EGL_EXT_platform_wayland".to_string(),
                                            "EGL_EXT_platform_device".to_string(),
                                        ],
                                        "1.5", "core", &mut file).unwrap();
    }

    if target.contains("ios") {
        let mut file = File::create(&dest.join("egl_bindings.rs")).unwrap();
        gl_generator::generate_bindings(gl_generator::StaticStructGenerator,
                                        gl_generator::registry::Ns::Egl,
                                        gl_generator::Fallbacks::All,
                                        khronos_api::EGL_XML,
                                        vec![
                                            "EGL_KHR_create_context".to_string(),
                                            "EGL_EXT_create_context_robustness".to_string(),
                                            "EGL_KHR_create_context_no_error".to_string(),
                                            "EGL_KHR_platform_x11".to_string(),
                                            "EGL_KHR_platform_android".to_string(),
                                            "EGL_KHR_platform_wayland".to_string(),
                                            "EGL_KHR_platform_gbm".to_string(),
                                            "EGL_EXT_platform_base".to_string(),
                                            "EGL_EXT_platform_x11".to_string(),
                                            "EGL_MESA_platform_gbm".to_string(),
                                            "EGL_EXT_platform_wayland".to_string(),
                                            "EGL_EXT_platform_device".to_string(),
                                        ],
                                        "1.5", "core", &mut file).unwrap();

        let mut file = File::create(&dest.join("gles2_bindings.rs")).unwrap();
        gl_generator::generate_bindings(gl_generator::StaticStructGenerator,
                                        gl_generator::registry::Ns::Gles2,
                                        gl_generator::Fallbacks::None,
                                        khronos_api::GL_XML,
                                        vec![],
                                        "2.0", "core", &mut file).unwrap();
    }

    if target.contains("darwin") {
        let mut file = File::create(&dest.join("gl_bindings.rs")).unwrap();
        gl_generator::generate_bindings(gl_generator::GlobalGenerator,
                                        gl_generator::registry::Ns::Gl,
                                        gl_generator::Fallbacks::All,
                                        khronos_api::GL_XML,
                                        vec!["GL_EXT_framebuffer_object".to_string()],
                                        "3.2", "core", &mut file).unwrap();
    }

    // TODO: only build the bindings below if we run tests/examples

    let mut file = File::create(&dest.join("test_gl_bindings.rs")).unwrap();
    gl_generator::generate_bindings(gl_generator::StructGenerator,
                                    gl_generator::registry::Ns::Gles2,
                                    gl_generator::Fallbacks::All,
                                    khronos_api::GL_XML, vec![],
                                    "3.0", "core", &mut file).unwrap();
}
