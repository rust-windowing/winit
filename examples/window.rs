extern crate init = "gl-init-rs";
extern crate libc;
extern crate gl;

fn main() {
    use std::default::Default;

    let window = init::Window::new(None, "Hello world!", &Default::default(), None).unwrap();

    unsafe { window.make_current() };

    gl::load_with(|symbol| window.get_proc_address(symbol) as *const libc::c_void);

    let version = {
        use std::c_str::CString;
        unsafe { CString::new(gl::GetString(gl::VERSION) as *const i8, false) }
    };

    println!("OpenGL version {}", version.as_str().unwrap());

    gl::ClearColor(0.0, 1.0, 0.0, 1.0);

    while !window.is_closed() {
        println!("{}", window.wait_events());

        gl::Clear(gl::COLOR_BUFFER_BIT);

        window.swap_buffers();
    }
}
