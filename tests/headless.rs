extern crate glutin;
extern crate libc;
use glutin::*;
use std::ptr;

mod gl {
    pub use self::Gles2 as Gl;
    include!(concat!(env!("OUT_DIR"), "/test_gl_bindings.rs"));
}
use gl::types::*;


#[cfg(target_os = "macos")]
#[test]
fn test_headless() {
    let width: i32 = 256;
    let height: i32 = 256;
    let window = glutin::HeadlessRendererBuilder::new(width as u32, height as u32).build().unwrap();

    unsafe { window.make_current() };

    let gl = gl::Gl::load_with(|symbol| window.get_proc_address(symbol) as *const _);

    unsafe {
        let mut framebuffer = 0;
        let mut texture = 0;
        gl.GenFramebuffers(1, &mut framebuffer);
        gl.BindFramebuffer(gl::FRAMEBUFFER, framebuffer);
        gl.GenTextures(1, &mut texture);
        gl.BindTexture(gl::TEXTURE_2D, texture);
        gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
        gl.TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
        gl.TexImage2D(gl::TEXTURE_2D, 0, gl::RGBA as i32, width, height,
                     0, gl::RGBA, gl::UNSIGNED_BYTE, ptr::null());
        gl.FramebufferTexture2D(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0, gl::TEXTURE_2D, texture, 0);
        let status = gl.CheckFramebufferStatus(gl::FRAMEBUFFER);
        if status != gl::FRAMEBUFFER_COMPLETE {
          panic!("Error while creating the framebuffer");
        }

        gl.ClearColor(0.0, 1.0, 0.0, 1.0);
        gl.Clear(gl::COLOR_BUFFER_BIT);
        gl.Enable(gl::SCISSOR_TEST);
        gl.Scissor(1, 0, 1, 1);
        gl.ClearColor(1.0, 0.0, 0.0, 1.0);
        gl.Clear(gl::COLOR_BUFFER_BIT);

        let mut values: Vec<u8> = vec![0;(width*height*4) as usize];
        gl.ReadPixels(0, 0, width, height, gl::RGBA, gl::UNSIGNED_BYTE, values.as_mut_ptr() as *mut GLvoid);

        assert_eq!(values[0], 0);
        assert_eq!(values[1], 255);
        assert_eq!(values[2], 0);
        assert_eq!(values[3], 255);

        assert_eq!(values[4], 255);
        assert_eq!(values[5], 0);
        assert_eq!(values[6], 0);
        assert_eq!(values[7], 255);
    }
}
