#![feature(phase)]
#![feature(tuple_indexing)]

#[phase(plugin)]
extern crate gl_generator;
extern crate glutin;
extern crate libc;

mod gl {
    generate_gl_bindings! {
        api: "gl",
        profile: "core",
        version: "1.1",
        generator: "struct"
    }
}

#[cfg(feature = "headless")]
#[test]
fn main() {
    let window = glutin::HeadlessRendererBuilder::new(1024, 768).build().unwrap();

    unsafe { window.make_current() };

    let gl = gl::Gl::load_with(|symbol| window.get_proc_address(symbol));

    gl.ClearColor(0.0, 1.0, 0.0, 1.0);
    gl.Clear(gl::COLOR_BUFFER_BIT);

    let mut value: (u8, u8, u8, u8) = unsafe { std::mem::uninitialized() };
    unsafe { gl.ReadPixels(0, 0, 1, 1, gl::RGBA, gl::UNSIGNED_BYTE, std::mem::transmute(&mut value)) };

    assert_eq!(value, (0, 255, 0, 255));
}
