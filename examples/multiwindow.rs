#[cfg(target_os = "android")]
#[macro_use]
extern crate android_glue;

extern crate glutin;

use std::thread;

mod support;

#[cfg(target_os = "android")]
android_start!(main);

#[cfg(not(feature = "window"))]
fn main() { println!("This example requires glutin to be compiled with the `window` feature"); }

#[cfg(feature = "window")]
fn main() {
    let window1 = glutin::Window::new().unwrap();
    let window2 = glutin::Window::new().unwrap();
    let window3 = glutin::Window::new().unwrap();

    let t1 = thread::spawn(move || {
        run(window1, (0.0, 1.0, 0.0, 1.0));
    });

    let t2 = thread::spawn(move || {
        run(window2, (0.0, 0.0, 1.0, 1.0));
    });

    let t3 = thread::spawn(move || {
        run(window3, (1.0, 0.0, 0.0, 1.0));
    });

    let _ = t1.join();
    let _ = t2.join();
    let _ = t3.join();
}

#[cfg(feature = "window")]
fn run(window: glutin::Window, color: (f32, f32, f32, f32)) {
    unsafe { window.make_current() };

    let context = support::load(&window);

    while !window.is_closed() {
        context.draw_frame(color);
        window.swap_buffers();

        window.wait_events().next();
    }
}
