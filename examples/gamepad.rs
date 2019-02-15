extern crate winit;
use winit::window::WindowBuilder;
use winit::event::{DeviceEvent, Event, WindowEvent};
use winit::event_loop::{EventLoop, ControlFlow};

fn main() {
    let mut event_loop = EventLoop::new();

    let _window = WindowBuilder::new()
        .with_title("The world's worst video game")
        .build(&event_loop)
        .unwrap();

    event_loop.run(|event, _, control_flow| {
        match event {
            Event::DeviceEvent { device_id, event } => match event {
                DeviceEvent::Button { .. }
                | DeviceEvent::Motion { .. } => {
                    println!("[{:?}] {:#?}", device_id, event);
                },
                _ => ()
            },
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => ()
        }
    });
}
