#![feature(phase)]

#[cfg(target_os = "android")]
#[phase(plugin, link)]
extern crate android_glue;

extern crate gl;
extern crate gl_init;
extern crate libc;

#[cfg(target_os = "android")]
android_start!(main)

fn main() {
    let window = gl_init::Window::new().unwrap();

    unsafe { window.make_current() };

    gl::load_with(|symbol| window.get_proc_address(symbol));

    let version = {
        use std::c_str::CString;
        unsafe { CString::new(gl::GetString(gl::VERSION) as *const i8, false) }
    };

    println!("OpenGL version {}", version.as_str().unwrap());

    {
        let win_size = window.get_inner_size().unwrap();
        gl::Viewport(0, 0, win_size.val0() as libc::c_int, win_size.val1() as libc::c_int);
    }

    gl::ClearColor(0.0, 1.0, 0.0, 1.0);

    while !window.is_closed() {
        gl::Clear(gl::COLOR_BUFFER_BIT);
        window.swap_buffers();

        println!("{}", window.wait_events().collect::<Vec<gl_init::Event>>());
    }
}
