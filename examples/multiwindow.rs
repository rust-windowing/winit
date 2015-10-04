#[cfg(target_os = "android")]
#[macro_use]
extern crate android_glue;

extern crate glutin;

use std::thread;

mod support;

#[cfg(target_os = "android")]
android_start!(main);

fn main() {
    let window1 = glutin::WindowBuilder::new().build().unwrap();
    let window2 = glutin::WindowBuilder::new().build().unwrap();
    let window3 = glutin::WindowBuilder::new().build().unwrap();

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

fn run(window: glutin::Window, color: (f32, f32, f32, f32)) {
    let _ = unsafe { window.make_current() };

    let context = support::load(&window);

    for event in window.wait_events() {
        context.draw_frame(color);
        let _ = window.swap_buffers();

        match event {
            glutin::Event::Closed => break,
            _ => ()
        }
    }
}
