extern crate winit;

use winit::{Event, WindowEvent, EventsLoop, ControlFlow};

fn create_window(title: &str, events_loop: &EventsLoop) -> winit::Window {
    let window_builder = winit::WindowBuilder::new()
        .with_title(title)
        .with_dimensions(300, 300)
        .with_maximized(false);
    window_builder        
        .build(events_loop)
        .unwrap()
}

fn main() {
    let mut events_loop = EventsLoop::new();

    let mut n_window = 0;
    let mut is_maximized = false;
    let mut decorations = true;
    
    let mut window = create_window("Hello world", &events_loop);

    let mut should_create_new_window = false;
    loop {
        let mut should_break = false;
        events_loop.run_forever(|event| {
            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::Closed => {
                        should_break = true;
                        ControlFlow::Break
                    },
                    WindowEvent::KeyboardInput {
                        input:
                            winit::KeyboardInput {
                                virtual_keycode: Some(virtual_code),
                                state,
                                ..
                            },
                        ..
                    } => match (virtual_code, state) {
                        (winit::VirtualKeyCode::Escape, _) => { 
                            should_break = true;
                            ControlFlow::Break
                        },
                        (winit::VirtualKeyCode::Return, winit::ElementState::Pressed) => {
                            should_create_new_window = true;
                            ControlFlow::Break
                        }
                        (winit::VirtualKeyCode::M, winit::ElementState::Pressed) => {
                            is_maximized = !is_maximized;
                            window.set_maximized(is_maximized);
                            ControlFlow::Continue
                        }
                        (winit::VirtualKeyCode::D, winit::ElementState::Pressed) => {
                            decorations = !decorations;
                            window.set_decorations(decorations);
                            ControlFlow::Continue
                        }
                        _ => ( ControlFlow::Continue ),
                    },
                    _ => ( ControlFlow::Continue ),
                },
                _ => { ControlFlow::Continue }
            }
        });
        if should_create_new_window {
            window = create_window(&format!("Hello world! {}", n_window), &events_loop);
            n_window += 1;
            should_create_new_window = false;   
        }
        if should_break {
            break;
        }
    }
}
