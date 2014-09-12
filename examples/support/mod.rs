#[phase(plugin)]
extern crate gl_generator;

use gl_init;

mod gl {
    generate_gl_bindings!("gl", "core", "4.5", "struct")
}

pub struct Context {
    gl: gl::Gl
}

pub fn load(window: &gl_init::Window) -> Context {
    let gl = gl::Gl::load_with(|symbol| window.get_proc_address(symbol));

    let version = {
        use std::c_str::CString;
        unsafe { CString::new(gl.GetString(gl::VERSION) as *const i8, false) }
    };

    println!("OpenGL version {}", version.as_str().unwrap());

    Context { gl: gl }
}

impl Context {
    pub fn draw_frame(&self, color: (f32, f32, f32, f32)) {
        self.gl.ClearColor(color.0, color.1, color.2, color.3);
        self.gl.Clear(gl::COLOR_BUFFER_BIT);
    }
}
