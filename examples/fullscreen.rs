extern crate winit;

use std::io::{self, Write};
use winit::monitor::MonitorHandle;
use winit::window::WindowBuilder;
use winit::event::{Event, WindowEvent, VirtualKeyCode, ElementState, KeyboardInput};
use winit::event_loop::{EventLoop, ControlFlow};

fn main() {
    let event_loop = EventLoop::new();

    #[cfg(target_os = "macos")]
    let mut macos_use_simple_fullscreen = false;

    let monitor = {
        // On macOS there are two fullscreen modes "native" and "simple"
        #[cfg(target_os = "macos")]
        {
            print!("Please choose the fullscreen mode: (1) native, (2) simple: ");
            io::stdout().flush().unwrap();

            let mut num = String::new();
            io::stdin().read_line(&mut num).unwrap();
            let num = num.trim().parse().ok().expect("Please enter a number");
            match num {
                2 => macos_use_simple_fullscreen = true,
                _ => {}
            }

            // Prompt for monitor when using native fullscreen
            if !macos_use_simple_fullscreen {
                Some(prompt_for_monitor(&event_loop))
            } else {
                None
            }
        }

        #[cfg(not(target_os = "macos"))]
        Some(prompt_for_monitor(&event_loop))
    };

    let mut is_fullscreen = monitor.is_some();
    let mut is_maximized = false;
    let mut decorations = true;

    let window = WindowBuilder::new()
        .with_title("Hello world!")
        .with_fullscreen(monitor)
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        println!("{:?}", event);
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(virtual_code),
                            state,
                            ..
                        },
                    ..
                } => match (virtual_code, state) {
                    (VirtualKeyCode::Escape, _) => *control_flow = ControlFlow::Exit,
                    (VirtualKeyCode::F, ElementState::Pressed) => {
                        #[cfg(target_os = "macos")]
                        {
                            if macos_use_simple_fullscreen {
                                use winit::platform::macos::WindowExtMacOS;
                                if WindowExtMacOS::set_simple_fullscreen(&window, !is_fullscreen) {
                                    is_fullscreen = !is_fullscreen;
                                }
                                return;
                            }
                        }

                        is_fullscreen = !is_fullscreen;
                        if !is_fullscreen {
                            window.set_fullscreen(None);
                        } else {
                            window.set_fullscreen(Some(window.get_current_monitor()));
                        }
                    }
                    (VirtualKeyCode::S, ElementState::Pressed) => {
                        println!("window.get_fullscreen {:?}", window.get_fullscreen());

                        #[cfg(target_os = "macos")]
                        {
                            use winit::platform::macos::WindowExtMacOS;
                            println!("window.get_simple_fullscreen {:?}", WindowExtMacOS::get_simple_fullscreen(&window));
                        }
                    }
                    (VirtualKeyCode::M, ElementState::Pressed) => {
                        is_maximized = !is_maximized;
                        window.set_maximized(is_maximized);
                    }
                    (VirtualKeyCode::D, ElementState::Pressed) => {
                        decorations = !decorations;
                        window.set_decorations(decorations);
                    }
                    _ => (),
                },
                _ => (),
            },
            _ => {}
        }
    });
}

// Enumerate monitors and prompt user to choose one
fn prompt_for_monitor(event_loop: &EventLoop<()>) -> MonitorHandle {
    for (num, monitor) in event_loop.get_available_monitors().enumerate() {
        println!("Monitor #{}: {:?}", num, monitor.get_name());
    }

    print!("Please write the number of the monitor to use: ");
    io::stdout().flush().unwrap();

    let mut num = String::new();
    io::stdin().read_line(&mut num).unwrap();
    let num = num.trim().parse().ok().expect("Please enter a number");
    let monitor = event_loop.get_available_monitors().nth(num).expect("Please enter a valid ID");

    println!("Using {:?}", monitor.get_name());

    monitor
}
