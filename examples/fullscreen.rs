#![allow(clippy::single_match)]

use simple_logger::SimpleLogger;
use winit::event::{ElementState, Event, KeyEvent, WindowEvent};
use winit::event_loop::EventLoop;
use winit::keyboard::Key;
use winit::window::{Fullscreen, WindowBuilder};

#[cfg(target_os = "macos")]
use winit::platform::macos::WindowExtMacOS;

#[path = "util/fill.rs"]
mod fill;

fn main() -> Result<(), impl std::error::Error> {
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
    #[cfg(target_os = "macos")]
    println!("- C\tToggle simple fullscreen mode");
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
                    event:
                        KeyEvent {
                            logical_key: key,
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => match key {
                    Key::Escape => control_flow.set_exit(),
                    // WARNING: Consider using `key_without_modifers()` if available on your platform.
                    // See the `key_binding` example
                    Key::Character(ch) => match ch.to_lowercase().as_str() {
                        "f" | "b" if window.fullscreen().is_some() => {
                            window.set_fullscreen(None);
                        }
                        "f" => {
                            let fullscreen = Some(Fullscreen::Exclusive(mode.clone()));
                            println!("Setting mode: {fullscreen:?}");
                            window.set_fullscreen(fullscreen);
                        }
                        "b" => {
                            let fullscreen = Some(Fullscreen::Borderless(Some(monitor.clone())));
                            println!("Setting mode: {fullscreen:?}");
                            window.set_fullscreen(fullscreen);
                        }
                        #[cfg(target_os = "macos")]
                        "c" => {
                            window.set_simple_fullscreen(!window.simple_fullscreen());
                        }
                        "s" => {
                            monitor_index += 1;
                            if let Some(mon) = elwt.available_monitors().nth(monitor_index) {
                                monitor = mon;
                            } else {
                                monitor_index = 0;
                                monitor =
                                    elwt.available_monitors().next().expect("no monitor found!");
                            }
                            println!("Monitor: {:?}", monitor.name());

                            mode_index = 0;
                            mode = monitor.video_modes().next().expect("no mode found");
                            println!("Mode: {mode}");
                        }
                        "m" => {
                            mode_index += 1;
                            if let Some(m) = monitor.video_modes().nth(mode_index) {
                                mode = m;
                            } else {
                                mode_index = 0;
                                mode = monitor.video_modes().next().expect("no mode found");
                            }
                            println!("Mode: {mode}");
                        }
                        "d" => {
                            decorations = !decorations;
                            window.set_decorations(decorations);
                        }
                        "x" => {
                            let is_maximized = window.is_maximized();
                            window.set_maximized(!is_maximized);
                        }
                        "z" => {
                            minimized = !minimized;
                            window.set_minimized(minimized);
                        }
                        _ => (),
                    },
                    _ => (),
                },
                _ => (),
            },
            Event::RedrawRequested(_) => {
                fill::fill_window(&window);
            }
            _ => {}
        }
    })
}
