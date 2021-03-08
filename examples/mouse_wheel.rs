use simple_logger::SimpleLogger;
use winit::{
    event::{DeviceEvent, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("Mouse Wheel events")
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                _ => (),
            },
            Event::DeviceEvent { event, .. } => match event {
                DeviceEvent::MouseWheel { delta } => match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => {
                        println!("mouse wheel Line Delta: ({},{})", x, y);
                        let pixels_per_line = 120.0;
                        let mut pos = window.outer_position().unwrap();
                        pos.x -= (x * pixels_per_line) as i32;
                        pos.y -= (y * pixels_per_line) as i32;
                        window.set_outer_position(pos)
                    }
                    winit::event::MouseScrollDelta::PixelDelta(p) => {
                        println!("mouse wheel Pixel Delta: ({},{})", p.x, p.y);
                        let mut pos = window.outer_position().unwrap();
                        pos.x -= p.x as i32;
                        pos.y -= p.y as i32;
                        window.set_outer_position(pos)
                    }
                },
                _ => (),
            },
            _ => (),
        }
    });
}
