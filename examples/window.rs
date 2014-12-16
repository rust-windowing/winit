#![feature(phase)]

#[cfg(target_os = "android")]
#[phase(plugin, link)]
extern crate android_glue;

extern crate glutin;

mod support;

#[cfg(target_os = "android")]
android_start!(main)

#[cfg(not(feature = "window"))]
fn main() { println!("This example requires glutin to be compiled with the `window` feature"); }

#[cfg(feature = "window")]
fn resize_callback(width: uint, height: uint) {
    println!("Window resized to {}x{}", width, height);
}

#[cfg(feature = "window")]
fn main() {
    let mut window = glutin::Window::new().unwrap();
    window.set_title("A fantastic window!");
    window.set_window_resize_callback(Some(resize_callback));
    unsafe { window.make_current() };

    let context = support::load(&window);

    while !window.is_closed() {
        context.draw_frame((0.0, 1.0, 0.0, 1.0));
        window.swap_buffers();

        println!("{}", window.wait_events().collect::<Vec<glutin::Event>>());
    }
}
