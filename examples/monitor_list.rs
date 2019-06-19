extern crate winit;
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    dbg!(window.available_monitors());
    dbg!(window.primary_monitor());
}
