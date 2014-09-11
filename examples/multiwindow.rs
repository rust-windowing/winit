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
    let window1 = gl_init::Window::new().unwrap();
    let window2 = gl_init::Window::new().unwrap();
    let window3 = gl_init::Window::new().unwrap();

    spawn(proc() {
        run(window1, (0.0, 1.0, 0.0, 1.0));
    });

    spawn(proc() {
        run(window2, (0.0, 0.0, 1.0, 1.0));
    });

    spawn(proc() {
        run(window3, (1.0, 0.0, 0.0, 1.0));
    });
}

fn run(window: gl_init::Window, color: (f32, f32, f32, f32)) {
    unsafe { window.make_current() };

    gl::load_with(|symbol| window.get_proc_address(symbol));
    
    {
        let win_size = window.get_inner_size().unwrap();
        gl::Viewport(0, 0, win_size.val0() as libc::c_int, win_size.val1() as libc::c_int);
    }

    gl::ClearColor(color.val0(), color.val1(), color.val2(), color.val3());

    while !window.is_closed() {
        gl::Clear(gl::COLOR_BUFFER_BIT);
        window.swap_buffers();

        window.wait_events().collect::<Vec<gl_init::Event>>();
    }
}
