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

    let context = support::load(&window);

    while !window.is_closed() {
        context.draw_frame(color);
        window.swap_buffers();

        window.wait_events().collect::<Vec<gl_init::Event>>();
    }
}
