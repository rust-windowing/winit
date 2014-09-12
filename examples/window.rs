#![feature(phase)]
#![feature(tuple_indexing)]

#[cfg(target_os = "android")]
#[phase(plugin, link)]
extern crate android_glue;

extern crate gl_init;

mod support;

#[cfg(target_os = "android")]
android_start!(main)

fn main() {
    let window = gl_init::Window::new().unwrap();

    unsafe { window.make_current() };

    let context = support::load(&window);

    while !window.is_closed() {
        context.draw_frame((0.0, 1.0, 0.0, 1.0));
        window.swap_buffers();

        println!("{}", window.wait_events().collect::<Vec<gl_init::Event>>());
    }
}
