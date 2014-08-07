extern crate init = "gl-init-rs";
extern crate libc;
extern crate gl;

fn main() {
    let window = init::Window::new().unwrap();

    unsafe { window.make_current() };

    gl::load_with(|symbol| window.get_proc_address(symbol) as *const libc::c_void);

    let version = {
        use std::c_str::CString;
        unsafe { CString::new(gl::GetString(gl::VERSION) as *const i8, false) }
    };

    println!("OpenGL version {}", version.as_str().unwrap());

    gl::ClearColor(0.0, 1.0, 0.0, 1.0);

    while !window.is_closed() {
        gl::Clear(gl::COLOR_BUFFER_BIT);
        window.swap_buffers();

        println!("{}", window.wait_events().collect::<Vec<init::Event>>());
    }
}
