#[cfg(target_os = "android")]
#[macro_use]
extern crate android_glue;

extern crate glutin;

mod support;

#[cfg(target_os = "android")]
android_start!(main);

fn resize_callback(width: u32, height: u32) {
    println!("Window resized to {}x{}", width, height);
}

fn main() {
    let mut window = glutin::WindowBuilder::new().with_decorations(false)
                                                 .with_transparency(true)
                                                 .build().unwrap();
    window.set_title("A fantastic window!");
    window.set_window_resize_callback(Some(resize_callback as fn(u32, u32)));
    let _ = unsafe { window.make_current() };

    println!("Pixel format of the window: {:?}", window.get_pixel_format());

    let context = support::load(&window);

    for event in window.wait_events() {
        context.draw_frame((0.0, 0.0, 0.0, 0.0));
        let _ = window.swap_buffers();

        println!("{:?}", event);

        match event {
            glutin::Event::Closed => break,
            _ => ()
        }
    }
}
