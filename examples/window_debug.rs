#![allow(clippy::single_match)]

// This example is used by developers to test various window functions.

use simple_logger::SimpleLogger;
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event::{DeviceEvent, ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{DeviceEventFilter, EventLoop},
    window::{Fullscreen, WindowBuilder},
};

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .with_inner_size(LogicalSize::new(100.0, 100.0))
        .build(&event_loop)
        .unwrap();

    eprintln!("debugging keys:");
    eprintln!("  (E) Enter exclusive fullscreen");
    eprintln!("  (F) Toggle borderless fullscreen");
    eprintln!("  (P) Toggle borderless fullscreen on system's preffered monitor");
    eprintln!("  (M) Toggle minimized");
    eprintln!("  (Q) Quit event loop");
    eprintln!("  (V) Toggle visibility");
    eprintln!("  (X) Toggle maximized");

    let mut minimized = false;
    let mut visible = true;

    event_loop.set_device_event_filter(DeviceEventFilter::Never);

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_wait();

        match event {
            Event::DeviceEvent {
                event:
                    DeviceEvent::Key(KeyboardInput {
                        virtual_keycode: Some(key),
                        state: ElementState::Pressed,
                        ..
                    }),
                ..
            } => match key {
                VirtualKeyCode::M => {
                    if minimized {
                        minimized = !minimized;
                        window.set_minimized(minimized);
                        window.focus_window();
                    }
                }
                VirtualKeyCode::V => {
                    if !visible {
                        visible = !visible;
                        window.set_visible(visible);
                    }
                }
                _ => (),
            },
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
                VirtualKeyCode::E => {
                    fn area(size: PhysicalSize<u32>) -> u32 {
                        size.width * size.height
                    }

                    let monitor = window.current_monitor().unwrap();
                    if let Some(mode) = monitor
                        .video_modes()
                        .max_by(|a, b| area(a.size()).cmp(&area(b.size())))
                    {
                        window.set_fullscreen(Some(Fullscreen::Exclusive(mode)));
                    } else {
                        eprintln!("no video modes available");
                    }
                }
                VirtualKeyCode::F => {
                    if window.fullscreen().is_some() {
                        window.set_fullscreen(None);
                    } else {
                        let monitor = window.current_monitor();
                        window.set_fullscreen(Some(Fullscreen::Borderless(monitor)));
                    }
                }
                VirtualKeyCode::P => {
                    if window.fullscreen().is_some() {
                        window.set_fullscreen(None);
                    } else {
                        window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                    }
                }
                VirtualKeyCode::M => {
                    minimized = !minimized;
                    window.set_minimized(minimized);
                }
                VirtualKeyCode::Q => {
                    control_flow.set_exit();
                }
                VirtualKeyCode::V => {
                    visible = !visible;
                    window.set_visible(visible);
                }
                VirtualKeyCode::X => {
                    let is_maximized = window.is_maximized();
                    window.set_maximized(!is_maximized);
                }
                _ => (),
            },
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => control_flow.set_exit(),
            _ => (),
        }
    });
}
