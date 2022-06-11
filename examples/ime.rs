#![allow(clippy::single_match)]

use log::LevelFilter;
use simple_logger::SimpleLogger;
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, Event, Ime, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::KeyCode,
    window::WindowBuilder,
};

fn main() {
    SimpleLogger::new()
        .with_level(LevelFilter::Trace)
        .init()
        .unwrap();

    println!("IME position will system default");
    println!("Click to set IME position to cursor's");
    println!("Press F2 to toggle IME. See the documentation of `set_ime_allowed` for more info");

    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_inner_size(winit::dpi::LogicalSize::new(256f64, 128f64))
        .build(&event_loop)
        .unwrap();

    let mut ime_allowed = true;
    window.set_ime_allowed(ime_allowed);

    let mut may_show_ime = false;
    let mut cursor_position = PhysicalPosition::new(0.0, 0.0);
    let mut ime_pos = PhysicalPosition::new(0.0, 0.0);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                ..
            } => {
                cursor_position = position;
            }
            Event::WindowEvent {
                event:
                    WindowEvent::MouseInput {
                        state: ElementState::Released,
                        ..
                    },
                ..
            } => {
                println!(
                    "Setting ime position to {}, {}",
                    cursor_position.x, cursor_position.y
                );
                ime_pos = cursor_position;
                if may_show_ime {
                    window.set_ime_position(ime_pos);
                }
            }
            Event::WindowEvent {
                event: WindowEvent::Ime(event),
                ..
            } => {
                println!("{:?}", event);
                may_show_ime = event != Ime::Disabled;
                if may_show_ime {
                    window.set_ime_position(ime_pos);
                }
            }
            Event::WindowEvent {
                event: WindowEvent::KeyboardInput { event, .. },
                ..
            } => {
                println!("key: {:?}", event);

                if event.state == ElementState::Pressed && event.physical_key == KeyCode::F2 {
                    ime_allowed = !ime_allowed;
                    window.set_ime_allowed(ime_allowed);
                    println!("\nIME: {}\n", ime_allowed);
                }
            }
            _ => (),
        }
    });
}
