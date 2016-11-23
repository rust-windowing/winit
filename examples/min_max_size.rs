extern crate winit;

fn main() {
    let window = winit::WindowBuilder::new()
        .with_min_dimensions(400, 200)
        .with_max_dimensions(800, 400)
        .build()
        .unwrap();

    for event in window.wait_events() {
        match event {
            winit::Event::Closed => break,
            _ => ()
        }
    }
}
