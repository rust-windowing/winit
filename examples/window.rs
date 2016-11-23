extern crate winit;

fn resize_callback(width: u32, height: u32) {
    println!("Window resized to {}x{}", width, height);
}

fn main() {
    let window = winit::WindowBuilder::new()
        .with_title("A fantastic window!")
        .with_window_resize_callback(resize_callback)
        .build()
        .unwrap();

    for event in window.wait_events() {
        println!("{:?}", event);

        match event {
            winit::Event::Closed => break,
            _ => ()
        }
    }
}
