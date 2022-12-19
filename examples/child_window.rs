#[cfg(x11_platform)]
use std::collections::HashMap;

#[cfg(x11_platform)]
use winit::{
    dpi::{LogicalPosition, LogicalSize, Position},
    event::{ElementState, Event, KeyboardInput, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
    platform::x11::{WindowBuilderExtX11, WindowExtX11},
    window::{Window, WindowBuilder, WindowId},
};

#[cfg(x11_platform)]
fn spawn_child_window(
    parent: u32,
    event_loop: &EventLoopWindowTarget<()>,
    windows: &mut HashMap<u32, Window>,
) {
    let child_window = WindowBuilder::new()
        .with_parent(WindowId::from(parent as u64))
        .with_title("child window")
        .with_inner_size(LogicalSize::new(200.0f32, 200.0f32))
        .with_position(Position::Logical(LogicalPosition::new(0.0, 0.0)))
        .with_visible(true)
        .build(event_loop)
        .unwrap();

    let id = child_window.xlib_window().unwrap() as u32;
    windows.insert(id, child_window);
    println!("child window created with id: {}", id);
}

#[cfg(x11_platform)]
fn main() {
    let mut windows = HashMap::new();

    let event_loop: EventLoop<()> = EventLoop::new();
    let parent_window = WindowBuilder::new()
        .with_title("parent window")
        .with_position(Position::Logical(LogicalPosition::new(0.0, 0.0)))
        .with_inner_size(LogicalSize::new(640.0f32, 480.0f32))
        .build(&event_loop)
        .unwrap();

    let root = parent_window.xlib_window().unwrap() as u32;
    println!("parent window id: {})", root);

    event_loop.run(move |event: Event<'_, ()>, event_loop, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Event::WindowEvent { event, window_id } = event {
            match event {
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
            }
        }
    })
}

#[cfg(not(x11_platform))]
fn main() {
    panic!("This example is supported only on x11.");
}
