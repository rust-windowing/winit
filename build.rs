extern crate gl_generator;
extern crate khronos_api;

use std::os;
use std::io::File;

fn main() {
    let target = os::getenv("TARGET").unwrap();
    let dest = Path::new(os::getenv("OUT_DIR").unwrap());

    if target.contains("windows") {
        let mut file = File::create(&dest.join("wgl_bindings.rs")).unwrap();
        gl_generator::generate_bindings(gl_generator::StaticGenerator,
                                        gl_generator::registry::Ns::Wgl,
                                        khronos_api::WGL_XML, vec![],
                                        "1.0", "core", &mut file).unwrap();

        let mut file = File::create(&dest.join("wgl_extra_bindings.rs")).unwrap();
        gl_generator::generate_bindings(gl_generator::StructGenerator,
                                        gl_generator::registry::Ns::Wgl,
                                        khronos_api::WGL_XML,
                                        vec![
                                            "WGL_ARB_create_context".to_string(),
                                            "WGL_EXT_swap_control".to_string()
                                        ],
                                        "1.0", "core", &mut file).unwrap();
    }

    if target.contains("linux") {
        let mut file = File::create(&dest.join("glx_bindings.rs")).unwrap();
        gl_generator::generate_bindings(gl_generator::StaticGenerator,
                                        gl_generator::registry::Ns::Glx,
                                        khronos_api::GLX_XML, vec![],
                                        "1.4", "core", &mut file).unwrap();

        let mut file = File::create(&dest.join("glx_extra_bindings.rs")).unwrap();
        gl_generator::generate_bindings(gl_generator::StructGenerator,
                                        gl_generator::registry::Ns::Glx,
                                        khronos_api::GLX_XML,
                                        vec![
                                            "GLX_ARB_create_context".to_string(),
                                        ],
                                        "1.4", "core", &mut file).unwrap();
    }

    if target.contains("android") {
        let mut file = File::create(&dest.join("egl_bindings.rs")).unwrap();
        gl_generator::generate_bindings(gl_generator::StaticGenerator,
                                        gl_generator::registry::Ns::Egl,
                                        khronos_api::EGL_XML, vec![],
                                        "1.5", "core", &mut file).unwrap();
    }
    

    // TODO: only build the bindings below if we run tests/examples

    let mut file = File::create(&dest.join("test_gl_bindings.rs")).unwrap();
    gl_generator::generate_bindings(gl_generator::StructGenerator,
                                    gl_generator::registry::Ns::Gl,
                                    khronos_api::GL_XML, vec![],
                                    "1.1", "core", &mut file).unwrap();

    let mut file = File::create(&dest.join("test_gles1_bindings.rs")).unwrap();
    gl_generator::generate_bindings(gl_generator::StructGenerator,
                                    gl_generator::registry::Ns::Gles1,
                                    khronos_api::GL_XML, vec![],
                                    "1.1", "core", &mut file).unwrap();
}
