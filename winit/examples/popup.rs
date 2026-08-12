//! This example shows the capabilities of popups
//! Move the mouse on the window and press 'A' to create a new Popup and 'D' to delete it again.
//! Move the Window to the border so see how the constraint adjustments behave like flipping or
//! sliding See `spawn_popup` and play with the various properties

#[cfg(any(x11_platform, macos_platform, windows_platform, wayland_platform))]
#[allow(deprecated)]
fn main() -> Result<(), impl std::error::Error> {
    use std::collections::HashMap;

    use softbuffer::{Context, Surface};
    use winit::application::ApplicationHandler;
    use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, Position};
    use winit::event::{ElementState, KeyEvent, WindowEvent};
    use winit::event_loop::{ActiveEventLoop, EventLoop, OwnedDisplayHandle};
    use winit::raw_window_handle::HasRawWindowHandle;
    use winit::window::{Window, WindowAttributes, WindowId};

    #[path = "util/fill.rs"]
    mod fill;

    #[derive(Debug)]
    struct WindowData {
        surface: Surface<OwnedDisplayHandle, Box<dyn Window>>,
        color: u32,
    }

    impl WindowData {
        fn new(context: &Context<OwnedDisplayHandle>, window: Box<dyn Window>, color: u32) -> Self {
            let surface = Surface::new(context, window).unwrap();
            Self { surface, color }
        }
    }

    #[derive(Debug)]
    struct Application {
        parent_window_id: Option<WindowId>,
        windows: HashMap<WindowId, WindowData>,
        popups: Vec<WindowId>,
        position: PhysicalPosition<f64>,
        context: Context<OwnedDisplayHandle>,
    }

    impl ApplicationHandler for Application {
        fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
            let attributes = WindowAttributes::default()
                .with_title("parent window")
                .with_position(Position::Logical(LogicalPosition::new(0.0, 0.0)))
                .with_surface_size(LogicalSize::new(600.0f32, 600.0f32))
                .with_decorations(true);
            let window = event_loop.create_window(attributes).unwrap();
            println!("Parent window id: {:?})", window.id());
            self.parent_window_id = Some(window.id());

            self.windows.insert(window.id(), WindowData::new(&self.context, window, 0xffbbbbbb));
        }

        fn window_event(
            &mut self,
            event_loop: &dyn ActiveEventLoop,
            window_id: winit::window::WindowId,
            event: WindowEvent,
        ) {
            use winit::keyboard::{KeyCode, PhysicalKey};

            // println!("Event: {:?}", event);
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
                    // println!("cursor entered in the window {window_id:?}");
                },
                WindowEvent::PointerMoved { position, .. } => {
                    self.position = position;
                    // println!("Physical position: {:?}", self.position);
                },
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            state: ElementState::Released,
                            physical_key: PhysicalKey::Code(code),
                            ..
                        },
                    ..
                } => {
                    match code {
                        KeyCode::KeyA => {
                            let window_id = if let Some(popup_id) = self.popups.last() {
                                popup_id
                            } else {
                                &self.parent_window_id.unwrap()
                            };

                            // Add a new Popup
                            let child_index = self.windows.len() - 1;
                            let child_color =
                                0xff000000 + 3_u32.pow((child_index + 2).rem_euclid(16) as u32);

                            let parent_window = self.windows.get(window_id).unwrap();
                            let child_window = spawn_popup(
                                parent_window.surface.window().as_ref(),
                                event_loop,
                                child_index,
                                self.position,
                            );
                            let child_id = child_window.id();
                            self.popups.push(child_id);
                            self.windows.insert(
                                child_id,
                                WindowData::new(&self.context, child_window, child_color),
                            );
                        },
                        KeyCode::KeyD => {
                            // Delete
                            if let Some(l) = self.popups.pop() {
                                self.windows.remove(&l);
                            }

                            // // When deleting the first, it should not lead to a wayland protocol
                            // // error
                            // if let Some(l) = self.popups.first() {
                            //     self.windows.remove(&l);
                            // }
                        },
                        _ => (),
                    }
                },
                WindowEvent::RedrawRequested => {
                    if let Some(window) = self.windows.get_mut(&window_id) {
                        if window_id == self.parent_window_id.unwrap() {
                            fill::fill(&mut window.surface);
                        } else {
                            fill::fill_with_color(&mut window.surface, window.color);
                        }
                    }
                },
                _ => (),
            }
        }
    }

    fn spawn_popup(
        parent: &dyn Window,
        event_loop: &dyn ActiveEventLoop,
        _child_count: usize,
        position: PhysicalPosition<f64>,
    ) -> Box<dyn Window> {
        use winit::dpi::Size;
        use winit::window::{
            WindowAnchor, WindowConstraintAdjustment, WindowGravity, WindowPositioner, WindowType,
        };

        let parent = parent.raw_window_handle().unwrap();

        let mut window_attributes = WindowAttributes::default()
            .with_title("child window")
            .with_surface_size(LogicalSize::new(300.0f32, 300.0))
            .with_decorations(false)
            .with_visible(true)
            .with_active(true) // Grab keyboard
            .with_window_type(WindowType::Popup)
            .with_positioner(WindowPositioner::new(
                WindowAnchor::TopLeft,
                (
                    Position::Physical(position.cast()),
                    Size::Logical(LogicalSize { width: 1., height: 1. }),
                ),
                Position::Logical(LogicalPosition { x: 0., y: 0. }),
                WindowGravity::BottomRight,
                WindowConstraintAdjustment::all(),
            ));

        // `with_parent_window` is unsafe. Parent window must be a valid window.
        window_attributes = unsafe { window_attributes.with_parent_window(Some(parent)) };

        event_loop.create_window(window_attributes).unwrap()
    }

    let event_loop = EventLoop::new().unwrap();
    let context = Context::new(event_loop.owned_display_handle()).unwrap();
    event_loop.run_app(Application {
        context,
        parent_window_id: None,
        windows: HashMap::new(),
        popups: Vec::default(),
        position: PhysicalPosition { x: 0., y: 0. },
    })
}

#[cfg(not(any(x11_platform, macos_platform, windows_platform, wayland_platform)))]
fn main() {
    panic!(
        "This example is supported only on wayland, x11, macOS, and Windows, with the `rwh_06` \
         feature enabled."
    );
}
