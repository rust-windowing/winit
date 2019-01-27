extern crate winit;

mod helpers;

fn main() {
    let event_loop = winit::EventsLoop::new();
    let window = winit::WindowBuilder::new().build(&event_loop).unwrap();
    helpers::init_wayland(&window);
    println!("{:#?}\nPrimary: {:#?}", window.get_available_monitors(), window.get_primary_monitor());
}
