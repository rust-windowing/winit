#![cfg(feature = "window")]

use std::ffi::CStr;
use glutin;

#[cfg(not(target_os = "android"))]
mod gl {
    include!(concat!(env!("OUT_DIR"), "/test_gl_bindings.rs"));
}

#[cfg(target_os = "android")]
mod gl {
    pub use self::Gles1 as Gl;
    include!(concat!(env!("OUT_DIR"), "/test_gles1_bindings.rs"));
}

pub struct Context {
    gl: gl::Gl
}

pub fn load(window: &glutin::Window) -> Context {
    let gl = gl::Gl::load(window);

    let version = unsafe {
        let data = CStr::from_ptr(gl.GetString(gl::VERSION) as *const i8).to_bytes().to_vec();
        String::from_utf8(data).unwrap()
    };

    println!("OpenGL version {}", version);

    Context { gl: gl }
}

impl Context {
    #[cfg(not(target_os = "android"))]
    pub fn draw_frame(&self, color: (f32, f32, f32, f32)) {
        unsafe {
            self.gl.ClearColor(color.0, color.1, color.2, color.3);
            self.gl.Clear(gl::COLOR_BUFFER_BIT);

            self.gl.Begin(gl::TRIANGLES);
            self.gl.Color3f(1.0, 0.0, 0.0);
            self.gl.Vertex2f(-0.5, -0.5);
            self.gl.Color3f(0.0, 1.0, 0.0);
            self.gl.Vertex2f(0.0, 0.5);
            self.gl.Color3f(0.0, 0.0, 1.0);
            self.gl.Vertex2f(0.5, -0.5);
            self.gl.End();

            self.gl.Flush();
        }
    }

    #[cfg(target_os = "android")]
    pub fn draw_frame(&self, color: (f32, f32, f32, f32)) {
        unsafe {
            self.gl.ClearColor(color.0, color.1, color.2, color.3);
            self.gl.Clear(gl::COLOR_BUFFER_BIT);

            self.gl.EnableClientState(gl::VERTEX_ARRAY);
            self.gl.EnableClientState(gl::COLOR_ARRAY);

            unsafe {
                use std::mem;
                self.gl.VertexPointer(2, gl::FLOAT, (mem::size_of::<f32>() * 5) as i32,
                    mem::transmute(VERTEX_DATA.as_slice().as_ptr()));
                self.gl.ColorPointer(3, gl::FLOAT, (mem::size_of::<f32>() * 5) as i32,
                    mem::transmute(VERTEX_DATA.as_slice().as_ptr().offset(2)));
            }

            self.gl.DrawArrays(gl::TRIANGLES, 0, 3);
            self.gl.DisableClientState(gl::VERTEX_ARRAY);
            self.gl.DisableClientState(gl::COLOR_ARRAY);
            
            self.gl.Flush();
        }
    }
}

#[cfg(target_os = "android")]
static VERTEX_DATA: [f32; 15] = [
    -0.5, -0.5, 1.0, 0.0, 0.0,
    0.0, 0.5, 0.0, 1.0, 0.0,
    0.5, -0.5, 0.0, 0.0, 1.0
];
