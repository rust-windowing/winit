#![allow(clippy::single_match)]

use simple_logger::SimpleLogger;
use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::Key,
    window::{Theme, Window},
};

#[path = "util/fill.rs"]
mod fill;

fn main() -> Result<(), impl std::error::Error> {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new().unwrap();

    let window = Window::builder()
        .with_title("A fantastic window!")
        .with_theme(Some(Theme::Dark))
        .build(&event_loop)
        .unwrap();

    println!("Initial theme: {:?}", window.theme());
    println!("debugging keys:");
    println!("  (A) Automatic theme");
    println!("  (L) Light theme");
    println!("  (D) Dark theme");

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Event::WindowEvent { window_id, event } = event {
            match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::ThemeChanged(theme) if window_id == window.id() => {
                    println!("Theme is changed: {theme:?}")
                }
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            logical_key: key,
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => match key.as_ref() {
                    Key::Character("A" | "a") => {
                        println!("Theme was: {:?}", window.theme());
                        window.set_theme(None);
                    }
                    Key::Character("L" | "l") => {
                        println!("Theme was: {:?}", window.theme());
                        window.set_theme(Some(Theme::Light));
                    }
                    Key::Character("D" | "d") => {
                        println!("Theme was: {:?}", window.theme());
                        window.set_theme(Some(Theme::Dark));
                    }
                    _ => (),
                },
                WindowEvent::RedrawRequested => {
                    println!("\nredrawing!\n");
                    fill::fill_window(&window);
                }
                _ => (),
            }
        }
    })
}
