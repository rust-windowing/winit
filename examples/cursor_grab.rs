#![allow(clippy::single_match)]

use simple_logger::SimpleLogger;
use winit::{
    event::{DeviceEvent, ElementState, Event, KeyboardInput, ModifiersState, WindowEvent},
    event_loop::EventLoop,
    window::{CursorGrabMode, WindowBuilder},
};

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("Super Cursor Grab'n'Hide Simulator 9000")
        .build(&event_loop)
        .unwrap();

    let mut modifiers = ModifiersState::default();

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_wait();

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => control_flow.set_exit(),
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            state: ElementState::Released,
                            virtual_keycode: Some(key),
                            ..
                        },
                    ..
                } => {
                    use winit::event::VirtualKeyCode::*;
                    match key {
                        Escape => control_flow.set_exit(),
                        G => window
                            .set_cursor_grab_mode(CursorGrabMode::Confined)
                            .unwrap(),
                        L => window.set_cursor_grab_mode(CursorGrabMode::Locked).unwrap(),
                        A => window.set_cursor_grab_mode(CursorGrabMode::None).unwrap(),
                        H => window.set_cursor_visible(modifiers.shift()),
                        _ => (),
                    }
                }
                WindowEvent::ModifiersChanged(m) => modifiers = m,
                _ => (),
            },
            Event::DeviceEvent { event, .. } => match event {
                DeviceEvent::MouseMotion { delta } => println!("mouse moved: {:?}", delta),
                DeviceEvent::Button { button, state } => match state {
                    ElementState::Pressed => println!("mouse button {} pressed", button),
                    ElementState::Released => println!("mouse button {} released", button),
                },
                _ => (),
            },
            _ => (),
        }
    });
}
