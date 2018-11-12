extern crate winit;

use winit::{ControlFlow, Event, WindowEvent};

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("The simple_fullscreen example only works on macOS");
}

#[cfg(target_os = "macos")]
fn main() {
    let mut events_loop = winit::EventsLoop::new();
    let window = winit::WindowBuilder::new()
        .with_title("Hello world!")
        .build(&events_loop)
        .unwrap();

    let mut is_simple_fullscreen = false;
    let mut is_maximized = false;
    let mut decorations = true;

    events_loop.run_forever(|event| {
        println!("{:?}", event);

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => return ControlFlow::Break,
                WindowEvent::KeyboardInput {
                    input:
                        winit::KeyboardInput {
                            virtual_keycode: Some(virtual_code),
                            state,
                            ..
                        },
                    ..
                } => match (virtual_code, state) {
                    (winit::VirtualKeyCode::Escape, _) => return ControlFlow::Break,
                    (winit::VirtualKeyCode::F, winit::ElementState::Pressed) => {
                        use winit::os::macos::WindowExt;

                        is_simple_fullscreen = !is_simple_fullscreen;
                        WindowExt::set_simple_fullscreen(&window, is_simple_fullscreen);
                    }
                    (winit::VirtualKeyCode::M, winit::ElementState::Pressed) => {
                        is_maximized = !is_maximized;
                        window.set_maximized(is_maximized);
                    }
                    (winit::VirtualKeyCode::D, winit::ElementState::Pressed) => {
                        decorations = !decorations;
                        window.set_decorations(decorations);
                    }
                    _ => (),
                },
                _ => (),
            },
            _ => {}
        }

        ControlFlow::Continue
    });
}
