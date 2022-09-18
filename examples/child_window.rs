#[cfg(all(target_os = "linux", feature = "x11"))]
use std::collections::HashMap;

#[cfg(all(target_os = "linux", feature = "x11"))]
use winit::{
    dpi::{LogicalPosition, LogicalSize, Position},
    event::{ElementState, Event, KeyboardInput, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
    platform::unix::{WindowBuilderExtUnix, WindowExtUnix},
    window::{Window, WindowBuilder},
};

#[cfg(all(target_os = "linux", feature = "x11"))]
fn spawn_child_window(
    parent: usize,
    event_loop: &EventLoopWindowTarget<()>,
    windows: &mut HashMap<usize, Window>,
) {
    let child_window = WindowBuilder::new()
        .with_x11_parent(parent)
        .with_title("child window")
        .with_inner_size(LogicalSize::new(200.0f32, 200.0f32))
        .with_position(Position::Logical(LogicalPosition::new(0.0, 0.0)))
        .with_visible(true)
        .build(&event_loop)
        .unwrap();

    let id: usize = child_window.xlib_window().unwrap().try_into().unwrap();
    windows.insert(id, child_window);
    println!("child window created with id: {}", id);
}

#[cfg(all(target_os = "linux", feature = "x11"))]
fn main() {
    let mut windows = HashMap::new();

    let event_loop: EventLoop<()> = EventLoop::new();
    let parent_window = WindowBuilder::new()
        .with_title("parent window")
        .with_position(Position::Logical(LogicalPosition::new(0.0, 0.0)))
        .with_inner_size(LogicalSize::new(640.0f32, 480.0f32))
        .build(&event_loop)
        .unwrap();
    let root: usize = parent_window.xlib_window().unwrap().try_into().unwrap();
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
                    // println when the cursor entered in a window even if the child window is created
                    // by some key inputs.
                    // the child windows are always placed at (0, 0) with size (200, 200) in the parent window,
                    // so we also can see this log when we move the cursor arround (200, 200) in parent window.
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
                    spawn_child_window(root, event_loop, &mut windows);
                }
                _ => (),
            },
            _ => (),
        }
    })
}

#[cfg(not(all(target_os = "linux", feature = "x11")))]
fn main() {
    panic!("This example is supported only on x11.");
}
