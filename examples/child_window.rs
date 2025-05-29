#[cfg(any(x11_platform, macos_platform, windows_platform))]
#[allow(deprecated)]
fn main() -> Result<(), impl std::error::Error> {
    use std::collections::HashMap;

    use winit::application::ApplicationHandler;
    use winit::dpi::{LogicalPosition, LogicalSize, Position};
    use winit::event::{ElementState, KeyEvent, WindowEvent};
    use winit::event_loop::{ActiveEventLoop, EventLoop};
    use winit::raw_window_handle::HasRawWindowHandle;
    use winit::window::{Window, WindowAttributes, SurfaceId};

    #[path = "util/fill.rs"]
    mod fill;

    #[derive(Debug)]
    struct WindowData {
        window: Box<dyn Window>,
        color: u32,
    }

    impl WindowData {
        fn new(window: Box<dyn Window>, color: u32) -> Self {
            Self { window, color }
        }
    }

    #[derive(Default, Debug)]
    struct Application {
        parent_window_id: Option<SurfaceId>,
        windows: HashMap<SurfaceId, WindowData>,
    }

    impl ApplicationHandler for Application {
        fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
            let attributes = WindowAttributes::default()
                .with_title("parent window")
                .with_position(Position::Logical(LogicalPosition::new(0.0, 0.0)))
                .with_surface_size(LogicalSize::new(640.0f32, 480.0f32));
            let window = event_loop.create_window(attributes).unwrap();
            println!("Parent window id: {:?})", window.id());
            self.parent_window_id = Some(window.id());

            self.windows.insert(window.id(), WindowData::new(window, 0xffbbbbbb));
        }

        fn window_event(
            &mut self,
            event_loop: &dyn ActiveEventLoop,
            window_id: winit::window::SurfaceId,
            event: WindowEvent,
        ) {
            match event {
                WindowEvent::CloseRequested => {
                    self.windows.clear();
                    event_loop.exit();
                },
                WindowEvent::PointerEntered { device_id: _, .. } => {
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
                    let child_index = self.windows.len() - 1;
                    let child_color =
                        0xff000000 + 3_u32.pow((child_index + 2).rem_euclid(16) as u32);

                    let parent_window = self.windows.get(&self.parent_window_id.unwrap()).unwrap();
                    let child_window =
                        spawn_child_window(parent_window.window.as_ref(), event_loop, child_index);
                    let child_id = child_window.id();
                    println!("Child window created with id: {child_id:?}");
                    self.windows.insert(child_id, WindowData::new(child_window, child_color));
                },
                WindowEvent::RedrawRequested => {
                    if let Some(window) = self.windows.get(&window_id) {
                        if window_id == self.parent_window_id.unwrap() {
                            fill::fill_window(window.window.as_ref());
                        } else {
                            fill::fill_window_with_color(window.window.as_ref(), window.color);
                        }
                    }
                },
                _ => (),
            }
        }
    }

    fn spawn_child_window(
        parent: &dyn Window,
        event_loop: &dyn ActiveEventLoop,
        child_count: usize,
    ) -> Box<dyn Window> {
        let parent = parent.raw_window_handle().unwrap();

        // As child count increases, x goes from 0*128 to 5*128 and then repeats
        let x: f64 = child_count.rem_euclid(5) as f64 * 128.0;

        // After 5 windows have been put side by side horizontally, a new row starts
        let y: f64 = (child_count / 5) as f64 * 96.0;

        let mut window_attributes = WindowAttributes::default()
            .with_title("child window")
            .with_surface_size(LogicalSize::new(128.0f32, 96.0))
            .with_position(Position::Logical(LogicalPosition::new(x, y)))
            .with_visible(true);
        // `with_parent_window` is unsafe. Parent window must be a valid window.
        window_attributes = unsafe { window_attributes.with_parent_window(Some(parent)) };

        event_loop.create_window(window_attributes).unwrap()
    }

    let event_loop = EventLoop::new().unwrap();
    event_loop.run_app(Application::default())
}

#[cfg(not(any(x11_platform, macos_platform, windows_platform)))]
fn main() {
    panic!(
        "This example is supported only on x11, macOS, and Windows, with the `rwh_06` feature \
         enabled."
    );
}
