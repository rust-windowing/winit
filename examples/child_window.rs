#[cfg(all(feature = "rwh_06", any(x11_platform, macos_platform, windows_platform)))]
#[allow(deprecated)]
fn main() -> Result<(), impl std::error::Error> {
    use std::collections::HashMap;

    use winit::application::ApplicationHandler;
    use winit::dpi::{LogicalPosition, LogicalSize, Position};
    use winit::event::{ElementState, KeyEvent, WindowEvent};
    use winit::event_loop::{ActiveEventLoop, EventLoop};
    use winit::raw_window_handle::HasRawWindowHandle;
    use winit::window::{Window, WindowId};

    #[path = "util/fill.rs"]
    mod fill;

    #[derive(Default)]
    struct Application {
        parent_window_id: Option<WindowId>,
        windows: HashMap<WindowId, Window>,
    }

    impl ApplicationHandler for Application {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            let attributes = Window::default_attributes()
                .with_title("parent window")
                .with_position(Position::Logical(LogicalPosition::new(0.0, 0.0)))
                .with_inner_size(LogicalSize::new(640.0f32, 480.0f32));
            let window = event_loop.create_window(attributes).unwrap();

            println!("Parent window id: {:?})", window.id());
            self.parent_window_id = Some(window.id());

            self.windows.insert(window.id(), window);
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            window_id: winit::window::WindowId,
            event: WindowEvent,
        ) {
            match event {
                WindowEvent::CloseRequested => {
                    self.windows.clear();
                    event_loop.exit();
                },
                WindowEvent::CursorEntered { device_id: _ } => {
                    // On x11, println when the cursor entered in a window even if the child window
                    // is created by some key inputs.
                    // the child windows are always placed at (0, 0) with size (200, 200) in the
                    // parent window, so we also can see this log when we move
                    // the cursor around (200, 200) in parent window.
                    println!("cursor entered in the window {window_id:?}");
                },
                WindowEvent::KeyboardInput {
                    event: KeyEvent { state: ElementState::Pressed, .. },
                    ..
                } => {
                    let parent_window = self.windows.get(&self.parent_window_id.unwrap()).unwrap();
                    let child_window = spawn_child_window(parent_window, event_loop);
                    let child_id = child_window.id();
                    println!("Child window created with id: {child_id:?}");
                    self.windows.insert(child_id, child_window);
                },
                WindowEvent::RedrawRequested => {
                    if let Some(window) = self.windows.get(&window_id) {
                        fill::fill_window(window);
                    }
                },
                _ => (),
            }
        }
    }

    fn spawn_child_window(parent: &Window, event_loop: &ActiveEventLoop) -> Window {
        let parent = parent.raw_window_handle().unwrap();
        let mut window_attributes = Window::default_attributes()
            .with_title("child window")
            .with_inner_size(LogicalSize::new(200.0f32, 200.0f32))
            .with_position(Position::Logical(LogicalPosition::new(0.0, 0.0)))
            .with_visible(true);
        // `with_parent_window` is unsafe. Parent window must be a valid window.
        window_attributes = unsafe { window_attributes.with_parent_window(Some(parent)) };

        event_loop.create_window(window_attributes).unwrap()
    }

    let event_loop: EventLoop<()> = EventLoop::new().unwrap();
    let mut app = Application::default();
    event_loop.run_app(&mut app)
}

#[cfg(all(feature = "rwh_06", not(any(x11_platform, macos_platform, windows_platform))))]
fn main() {
    panic!(
        "This example is supported only on x11, macOS, and Windows, with the `rwh_06` feature \
         enabled."
    );
}
