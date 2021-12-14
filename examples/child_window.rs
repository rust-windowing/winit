use std::collections::HashMap;

use simple_logger::SimpleLogger;
use winit::{
    dpi::{LogicalPosition, LogicalSize, Position},
    event::{ElementState, Event, KeyboardInput, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

#[cfg(feature = "x11")]
use winit::platform::unix::{WindowBuilderExtUnix, WindowExtUnix};

#[cfg(feature = "x11")]
fn main() {
    SimpleLogger::new().init().unwrap();
    let mut windows = HashMap::new();

    let event_loop: EventLoop<()> = EventLoop::new();
    let parent_window = WindowBuilder::new()
        .with_title("parent window")
        .with_position(Position::Logical(LogicalPosition::new(0.0, 0.0)))
        .with_inner_size(LogicalSize::new(640.0, 480.0))
        .build(&event_loop)
        .unwrap();
    let root = parent_window.xlib_window().unwrap();
    println!("parent window id: {})", root);

    event_loop.run(move |event: Event<'_, ()>, event_loop, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent { event, window_id } => match event {
                WindowEvent::CloseRequested => {
                    windows.clear();
                    *control_flow = ControlFlow::Exit;
                }
                WindowEvent::CursorEntered { device_id: _ } => {
                    println!("cursor entered in the window {:?}", window_id);
                }
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => {
                    let child_window = WindowBuilder::new()
                        .with_x11_parent(root)
                        .with_title("child window")
                        .with_inner_size(LogicalSize::new(100.0, 100.0))
                        .build(&event_loop)
                        .unwrap();
                    println!(
                        "child window created with id: {}",
                        child_window.xlib_window().unwrap()
                    );
                    windows.insert(child_window.id(), child_window);
                }
                _ => (),
            },
            _ => (),
        }
    })
}

#[cfg(not(feature = "x11"))]
fn main() {
    panic!("This example is supported only on x11.");
}
