#[cfg(any(x11_platform, macos_platform, windows_platform))]
fn main() {
    use std::collections::HashMap;

    use raw_window_handle::HasRawWindowHandle;
    use winit::{
        dpi::{LogicalPosition, LogicalSize, Position},
        event::{ElementState, Event, KeyboardInput, WindowEvent},
        event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
        window::{Window, WindowBuilder, WindowId},
    };

    fn spawn_child_window(
        parent: &Window,
        event_loop: &EventLoopWindowTarget<()>,
        windows: &mut HashMap<WindowId, Window>,
    ) {
        let parent = parent.raw_window_handle();
        let mut builder = WindowBuilder::new()
            .with_title("child window")
            .with_inner_size(LogicalSize::new(200.0f32, 200.0f32))
            .with_position(Position::Logical(LogicalPosition::new(0.0, 0.0)))
            .with_visible(true);
        // `with_parent_window` is unsafe. Parent window must be a valid window.
        builder = unsafe { builder.with_parent_window(Some(parent)) };
        let child_window = builder.build(event_loop).unwrap();

        let id = child_window.id();
        windows.insert(id, child_window);
        println!("child window created with id: {id:?}");
    }

    let mut windows = HashMap::new();

    let event_loop: EventLoop<()> = EventLoop::new();
    let parent_window = WindowBuilder::new()
        .with_title("parent window")
        .with_position(Position::Logical(LogicalPosition::new(0.0, 0.0)))
        .with_inner_size(LogicalSize::new(640.0f32, 480.0f32))
        .build(&event_loop)
        .unwrap();

    println!("parent window: {parent_window:?})");

    event_loop.run(move |event: Event<'_, ()>, event_loop, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Event::WindowEvent { event, window_id } = event {
            match event {
                WindowEvent::CloseRequested => {
                    windows.clear();
                    *control_flow = ControlFlow::Exit;
                }
                WindowEvent::CursorEntered { device_id: _ } => {
                    // On x11, println when the cursor entered in a window even if the child window is created
                    // by some key inputs.
                    // the child windows are always placed at (0, 0) with size (200, 200) in the parent window,
                    // so we also can see this log when we move the cursor arround (200, 200) in parent window.
                    println!("cursor entered in the window {window_id:?}");
                }
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => {
                    spawn_child_window(&parent_window, event_loop, &mut windows);
                }
                _ => (),
            }
        }
    })
}

#[cfg(not(any(x11_platform, macos_platform, windows_platform)))]
fn main() {
    panic!("This example is supported only on x11, macOS, and Windows.");
}
