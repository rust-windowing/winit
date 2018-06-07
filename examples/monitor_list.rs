extern crate winit;

fn main() {
    let event_loop = winit::EventsLoop::new();
    let window = winit::WindowBuilder::new().build(&event_loop).unwrap();
    println!("{:#?}\nPrimary: {:#?}", window.get_available_monitors(), window.get_primary_monitor());
}
