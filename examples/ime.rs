#![allow(clippy::single_match)]

use log::LevelFilter;
use simple_logger::SimpleLogger;
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{ElementState, Event, Ime, WindowEvent},
    event_loop::EventLoop,
    keyboard::NamedKey,
    window::{ImePurpose, WindowBuilder},
};

#[path = "util/fill.rs"]
mod fill;

fn main() -> Result<(), impl std::error::Error> {
    SimpleLogger::new()
        .with_level(LevelFilter::Trace)
        .init()
        .unwrap();

    println!("IME position will system default");
    println!("Click to set IME position to cursor's");
    println!("Press F2 to toggle IME. See the documentation of `set_ime_allowed` for more info");
    println!("Press F3 to cycle through IME purposes.");

    let event_loop = EventLoop::new().unwrap();

    let window = WindowBuilder::new()
        .with_inner_size(winit::dpi::LogicalSize::new(256f64, 128f64))
        .build(&event_loop)
        .unwrap();

    let mut ime_purpose = ImePurpose::Normal;
    let mut ime_allowed = true;
    window.set_ime_allowed(ime_allowed);

    let mut may_show_ime = false;
    let mut cursor_position = PhysicalPosition::new(0.0, 0.0);
    let mut ime_pos = PhysicalPosition::new(0.0, 0.0);

    event_loop.run(move |event, elwt| {
        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::CursorMoved { position, .. } => {
                    cursor_position = position;
                }
                WindowEvent::MouseInput {
                    state: ElementState::Released,
                    ..
                } => {
                    println!(
                        "Setting ime position to {}, {}",
                        cursor_position.x, cursor_position.y
                    );
                    ime_pos = cursor_position;
                    if may_show_ime {
                        window.set_ime_cursor_area(ime_pos, PhysicalSize::new(10, 10));
                    }
                }
                WindowEvent::Ime(event) => {
                    println!("{event:?}");
                    may_show_ime = event != Ime::Disabled;
                    if may_show_ime {
                        window.set_ime_cursor_area(ime_pos, PhysicalSize::new(10, 10));
                    }
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    println!("key: {event:?}");

                    if event.state == ElementState::Pressed && event.logical_key == NamedKey::F2 {
                        ime_allowed = !ime_allowed;
                        window.set_ime_allowed(ime_allowed);
                        println!("\nIME allowed: {ime_allowed}\n");
                    }
                    if event.state == ElementState::Pressed && event.logical_key == NamedKey::F3 {
                        ime_purpose = match ime_purpose {
                            ImePurpose::Normal => ImePurpose::Password,
                            ImePurpose::Password => ImePurpose::Terminal,
                            _ => ImePurpose::Normal,
                        };
                        window.set_ime_purpose(ime_purpose);
                        println!("\nIME purpose: {ime_purpose:?}\n");
                    }
                }
                WindowEvent::RedrawRequested => {
                    fill::fill_window(&window);
                }
                _ => (),
            }
        }
    })
}
