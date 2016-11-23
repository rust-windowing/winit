extern crate winit;

fn resize_callback(width: u32, height: u32) {
    println!("Window resized to {}x{}", width, height);
}

fn main() {
    let mut window = winit::WindowBuilder::new().with_decorations(false)
                                                 .with_transparency(true)
                                                 .build().unwrap();
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
