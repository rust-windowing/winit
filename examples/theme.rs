#![allow(clippy::single_match)]

use simple_logger::SimpleLogger;
use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Theme, WindowBuilder},
};

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
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

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            Event::WindowEvent {
                event: WindowEvent::ThemeChanged(theme),
                window_id,
                ..
            } if window_id == window.id() => {
                println!("Theme is changed: {theme:?}")
            }
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(key),
                                state: ElementState::Pressed,
                                ..
                            },
                        ..
                    },
                ..
            } => match key {
                VirtualKeyCode::A => {
                    println!("Theme was: {:?}", window.theme());
                    window.set_theme(None);
                }
                VirtualKeyCode::L => {
                    println!("Theme was: {:?}", window.theme());
                    window.set_theme(Some(Theme::Light));
                }
                VirtualKeyCode::D => {
                    println!("Theme was: {:?}", window.theme());
                    window.set_theme(Some(Theme::Dark));
                }
                _ => (),
            },
            _ => (),
        }
    });
}
