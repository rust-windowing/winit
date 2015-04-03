#[cfg(target_os = "android")]
#[macro_use]
extern crate android_glue;

extern crate clock_ticks;
extern crate glutin;

mod support;

#[cfg(target_os = "android")]
android_start!(main);

#[cfg(not(feature = "window"))]
fn main() { println!("This example requires glutin to be compiled with the `window` feature"); }

#[cfg(feature = "window")]
fn resize_callback(width: u32, height: u32) {
    println!("Window resized to {}x{}", width, height);
}

#[cfg(feature = "window")]
fn main() {
    println!("Vsync example. This example may panic if your driver or your system forces \
              you out of vsync. This is intended when `build_strict` is used.");

    let mut window = glutin::WindowBuilder::new().with_vsync().build_strict().unwrap();
    window.set_window_resize_callback(Some(resize_callback as fn(u32, u32)));
    unsafe { window.make_current() };

    let context = support::load(&window);

    while !window.is_closed() {
        let before = clock_ticks::precise_time_ns();

        context.draw_frame((0.0, 1.0, 0.0, 1.0));
        window.swap_buffers();

        for ev in window.poll_events() {
            println!("{:?}", ev);
        }

        let after = clock_ticks::precise_time_ns();
        println!("Vsync example - Time of previous frame: {}ms",
                 (after - before) as f32 / 1000000.0);
    }
}
