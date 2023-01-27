#![allow(clippy::single_match)]

use simple_logger::SimpleLogger;
use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::{Fullscreen, WindowBuilder};

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let mut decorations = true;
    let mut minimized = false;

    let window = WindowBuilder::new()
        .with_title("Hello world!")
        .build(&event_loop)
        .unwrap();

    let mut monitor_index = 0;
    let mut monitor = event_loop
        .available_monitors()
        .next()
        .expect("no monitor found!");
    println!("Monitor: {:?}", monitor.name());

    let mut mode_index = 0;
    let mut mode = monitor.video_modes().next().expect("no mode found");
    println!("Mode: {mode}");

    println!("Keys:");
    println!("- Esc\tExit");
    println!("- F\tToggle exclusive fullscreen mode");
    println!("- B\tToggle borderless mode");
    println!("- S\tNext screen");
    println!("- M\tNext mode for this screen");
    println!("- D\tToggle window decorations");
    println!("- X\tMaximize window");
    println!("- Z\tMinimize window");

    event_loop.run(move |event, elwt, control_flow| {
        control_flow.set_wait();

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => control_flow.set_exit(),
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(virtual_code),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => match virtual_code {
                    VirtualKeyCode::Escape => control_flow.set_exit(),
                    VirtualKeyCode::F | VirtualKeyCode::B if window.fullscreen().is_some() => {
                        window.set_fullscreen(None);
                    }
                    VirtualKeyCode::F => {
                        let fullscreen = Some(Fullscreen::Exclusive(mode.clone()));
                        println!("Setting mode: {fullscreen:?}");
                        window.set_fullscreen(fullscreen);
                    }
                    VirtualKeyCode::B => {
                        let fullscreen = Some(Fullscreen::Borderless(Some(monitor.clone())));
                        println!("Setting mode: {fullscreen:?}");
                        window.set_fullscreen(fullscreen);
                    }
                    VirtualKeyCode::S => {
                        monitor_index += 1;
                        if let Some(mon) = elwt.available_monitors().nth(monitor_index) {
                            monitor = mon;
                        } else {
                            monitor_index = 0;
                            monitor = elwt.available_monitors().next().expect("no monitor found!");
                        }
                        println!("Monitor: {:?}", monitor.name());

                        mode_index = 0;
                        mode = monitor.video_modes().next().expect("no mode found");
                        println!("Mode: {mode}");
                    }
                    VirtualKeyCode::M => {
                        mode_index += 1;
                        if let Some(m) = monitor.video_modes().nth(mode_index) {
                            mode = m;
                        } else {
                            mode_index = 0;
                            mode = monitor.video_modes().next().expect("no mode found");
                        }
                        println!("Mode: {mode}");
                    }
                    VirtualKeyCode::D => {
                        decorations = !decorations;
                        window.set_decorations(decorations);
                    }
                    VirtualKeyCode::X => {
                        let is_maximized = window.is_maximized();
                        window.set_maximized(!is_maximized);
                    }
                    VirtualKeyCode::Z => {
                        minimized = !minimized;
                        window.set_minimized(minimized);
                    }
                    _ => (),
                },
                _ => (),
            },
            _ => {}
        }
    });
}
