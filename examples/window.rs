#[cfg(target_os = "android")]
#[macro_use]
extern crate android_glue;

extern crate winit;

#[cfg(target_os = "android")]
android_start!(main);

fn resize_callback(width: u32, height: u32) {
    println!("Window resized to {}x{}", width, height);
}

fn main() {
    let mut window = winit::WindowBuilder::new().build().unwrap();
    window.set_title("A fantastic window!");
    window.set_window_resize_callback(Some(resize_callback as fn(u32, u32)));

    for event in window.wait_events() {
        println!("{:?}", event);

        match event {
            winit::Event::Closed => break,
            _ => ()
        }
    }
}
