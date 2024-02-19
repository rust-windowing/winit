#[cfg(all(
    feature = "rwh_06",
    any(x11_platform, wayland_platform, macos_platform, windows_platform)
))]
#[path = "util/fill.rs"]
mod fill;

#[cfg(all(
    feature = "rwh_06",
    any(x11_platform, wayland_platform, macos_platform, windows_platform)
))]
#[allow(deprecated)]
fn main() -> Result<(), impl std::error::Error> {
    use std::collections::HashMap;

    use rwh_06::HasRawWindowHandle;
    use winit::{
        dpi::{LogicalPosition, LogicalSize},
        event::{ElementState, Event, KeyEvent, WindowEvent},
        event_loop::{EventLoop, EventLoopWindowTarget},
        keyboard::{KeyCode, PhysicalKey},
        window::{Window, WindowId},
    };

    fn spawn_child(
        parent: &Window,
        event_loop: &EventLoopWindowTarget,
        children: &mut HashMap<WindowId, Window>,
    ) {
        let mut builder = Window::builder()
            .with_position(LogicalPosition::new(0, 0))
            .with_inner_size(LogicalSize::new(200.0f32, 200.0f32))
            .with_visible(true);

        builder = unsafe { builder.with_parent_window(parent.raw_window_handle().ok()) };

        let child = builder.build(event_loop).unwrap();
        let child_id = child.id();

        children.insert(child_id, child);
    }

    let event_loop = EventLoop::new().unwrap();

    let window = Window::builder()
        .with_title("parent window")
        .with_inner_size(winit::dpi::LogicalSize::new(640.0, 480.0))
        .build(&event_loop)
        .unwrap();
    let mut children = HashMap::<WindowId, Window>::new();

    event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent { event, window_id } => match event {
                WindowEvent::CloseRequested => {
                    children.clear();
                    elwt.exit();
                }
                WindowEvent::RedrawRequested => {
                    // Notify the windowing system that we'll be presenting to the window.
                    if let Some(child_window) = children.get(&window_id) {
                        child_window.pre_present_notify();
                        fill::fill_window(&child_window);
                    } else if window_id == window.id() {
                        window.pre_present_notify();
                        fill::fill_window(&window);
                    }
                }
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            physical_key: PhysicalKey::Code(KeyCode::Enter),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => {
                    println!("Spawning child...");
                    spawn_child(&window, &elwt, &mut children);
                }
                WindowEvent::CursorEntered { .. } => {
                    println!("cursor entered window {window_id:?}");
                }
                _ => (),
            },
            Event::AboutToWait => {
                window.request_redraw();
            }

            _ => (),
        }
    })
}

#[cfg(not(all(
    feature = "rwh_06",
    any(x11_platform, wayland_platform, macos_platform, windows_platform)
)))]
fn main() {
    panic!("This example is supported only on X11, Wayland, macOS, and Windows, with the `rwh_06` feature enabled.");
}
