//! Simple winit application.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::fmt::Debug;
#[cfg(not(any(android_platform, ios_platform)))]
use std::num::NonZeroU32;
use std::path::Path;

use cursor_icon::CursorIcon;
#[cfg(not(any(android_platform, ios_platform)))]
use rwh_05::HasRawDisplayHandle;
#[cfg(not(any(android_platform, ios_platform)))]
use softbuffer::{Context, Surface};

use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{DeviceEvent, DeviceId, ElementState, Event, Ime, KeyEvent, WindowEvent};
use winit::event::{MouseButton, MouseScrollDelta};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState};
use winit::window::{
    Cursor, CursorGrabMode, CustomCursor, CustomCursorSource, Fullscreen, Icon, ResizeDirection,
    Theme,
};
use winit::window::{Window, WindowId};

#[cfg(macos_platform)]
use winit::platform::macos::{OptionAsAlt, WindowAttributesExtMacOS, WindowExtMacOS};
#[cfg(any(x11_platform, wayland_platform))]
use winit::platform::startup_notify::{
    self, EventLoopExtStartupNotify, WindowAttributesExtStartupNotify, WindowExtStartupNotify,
};

/// The amount of points to around the window for drag resize direction calculations.
const BORDER_SIZE: f64 = 20.;

fn main() -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    let _event_loop_proxy = event_loop.create_proxy();

    // Wire the user event from another thread.
    #[cfg(not(web_platform))]
    std::thread::spawn(move || {
        // Wake up the `event_loop` once every second and dispatch a custom event
        // from a different thread.
        println!("Starting to send user event every second");
        loop {
            let _ = _event_loop_proxy.send_event(UserEvent::WakeUp);
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    });

    let mut app = Application::new(&event_loop);

    event_loop.run(move |event, event_loop| match event {
        Event::NewEvents(_) => (),
        Event::Resumed => {
            println!("Resumed the event loop");

            // Create initial window.
            app.create_window(event_loop, None)
                .expect("failed to create initial window");

            app.print_help();
        }
        Event::AboutToWait => {
            if app.windows.is_empty() {
                println!("No windows left, exiting...");
                event_loop.exit();
            }
        }
        Event::WindowEvent { window_id, event } => {
            app.handle_window_event(event_loop, window_id, event)
        }
        Event::DeviceEvent { device_id, event } => {
            app.handle_device_event(event_loop, device_id, event)
        }
        Event::UserEvent(event) => {
            println!("User event: {event:?}");
        }
        Event::Suspended | Event::LoopExiting | Event::MemoryWarning => (),
    })?;

    Ok(())
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
enum UserEvent {
    WakeUp,
}

/// Application state and event handling.
struct Application {
    /// Custom cursors assets.
    custom_cursors: Vec<CustomCursor>,
    /// Application icon.
    icon: Icon,
    windows: HashMap<WindowId, WindowState>,
    /// Drawing context.
    ///
    /// With OpenGL it could be EGLDisplay.
    #[cfg(not(any(android_platform, ios_platform)))]
    context: Context,
}

impl Application {
    fn new<T>(event_loop: &EventLoop<T>) -> Self {
        // SAFETY: the context is dropped inside the loop, since the state we're using
        // is moved inside the closure.
        #[cfg(not(any(android_platform, ios_platform)))]
        let context = unsafe { Context::from_raw(event_loop.raw_display_handle()).unwrap() };

        // You'll have to choose an icon size at your own discretion. On X11, the desired size varies
        // by WM, and on Windows, you still have to account for screen scaling. Here we use 32px,
        // since it seems to work well enough in most cases. Be careful about going too high, or
        // you'll be bitten by the low-quality downscaling built into the WM.
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/data/icon.png");

        let icon = load_icon(Path::new(path));

        println!("Loading cursor assets");
        let custom_cursors = vec![
            event_loop.create_custom_cursor(decode_cursor(include_bytes!("data/cross.png"))),
            event_loop.create_custom_cursor(decode_cursor(include_bytes!("data/cross2.png"))),
            event_loop.create_custom_cursor(decode_cursor(include_bytes!("data/gradient.png"))),
        ];

        Self {
            #[cfg(not(any(android_platform, ios_platform)))]
            context,
            custom_cursors,
            icon,
            windows: Default::default(),
        }
    }

    fn create_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        _tab_id: Option<String>,
    ) -> Result<WindowId, Box<dyn Error>> {
        // TODO read-out activation token.

        #[allow(unused_mut)]
        let mut window_attributes = Window::default_attributes()
            .with_title("Winit window")
            .with_transparent(true)
            .with_window_icon(Some(self.icon.clone()));

        #[cfg(any(x11_platform, wayland_platform))]
        if let Some(token) = event_loop.read_token_from_env() {
            startup_notify::reset_activation_token_env();
            println!("Using token {:?} to activate a window", token);
            window_attributes = window_attributes.with_activation_token(token);
        }

        #[cfg(macos_platform)]
        if let Some(tab_id) = _tab_id {
            window_attributes = window_attributes.with_tabbing_identifier(&tab_id);
        }

        let window = event_loop.create_window(window_attributes)?;

        #[cfg(ios_platform)]
        {
            use winit::platform::ios::WindowExtIOS;
            window.recognize_doubletap_gesture(true);
            window.recognize_pinch_gesture(true);
            window.recognize_rotation_gesture(true);
        }

        #[cfg(not(any(android_platform, ios_platform)))]
        let surface = {
            // SAFETY: the surface is dropped before the `window` which
            // provided it with handle, thus it doesn't outlive it.
            let mut surface = unsafe { Surface::new(&self.context, &window)? };

            let size = window.inner_size();
            if let (Some(width), Some(height)) =
                (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
            {
                surface
                    .resize(width, height)
                    .expect("failed to resize inner buffer");
            };

            surface
        };

        let theme = window.theme().unwrap_or(Theme::Dark);
        println!("Theme: {theme:?}");
        let named_idx = 0;
        window.set_cursor(CURSORS[named_idx]);

        // Allow IME out of the box.
        let ime = true;
        window.set_ime_allowed(ime);

        let state = WindowState {
            window,
            custom_idx: self.custom_cursors.len() - 1,
            cursor_grab: CursorGrabMode::None,
            named_idx,
            #[cfg(not(any(android_platform, ios_platform)))]
            surface,
            theme,
            ime,
            cursor_position: Default::default(),
            cursor_hidden: Default::default(),
            modifiers: Default::default(),
            occluded: Default::default(),
            rotated: Default::default(),
            zoom: Default::default(),
        };

        let window_id = state.window.id();
        println!("Created new window with id={window_id:?}");
        self.windows.insert(window_id, state);
        Ok(window_id)
    }

    fn handle_action(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, action: Action) {
        let state = self.windows.get_mut(&window_id).unwrap();
        let window = &state.window;

        println!("Executing action: {action:?}");
        match action {
            Action::CloseWindow => {
                let _ = self.windows.remove(&window_id);
            }
            Action::CreateNewWindow => {
                #[cfg(any(x11_platform, wayland_platform))]
                if let Err(err) = window.request_activation_token() {
                    println!("Failed to get activation token: {err}");
                } else {
                    return;
                }

                if let Err(err) = self.create_window(event_loop, None) {
                    eprintln!("Error creating new window: {err}");
                }
            }
            Action::ToggleResizeIncrements => {
                let new_increments = match window.resize_increments() {
                    Some(_) => None,
                    None => Some(LogicalSize::new(25.0, 25.0)),
                };
                println!("Had increments: {}", new_increments.is_none());
                window.set_resize_increments(new_increments);
            }
            Action::ToggleCursorVisibility => {
                state.cursor_hidden = !state.cursor_hidden;
                window.set_cursor_visible(!state.cursor_hidden);
            }
            Action::ToggleResizable => {
                let resizable = window.is_resizable();
                window.set_resizable(!resizable);
            }
            Action::ToggleDecorations => {
                let decorated = window.is_decorated();
                window.set_decorations(!decorated);
            }
            Action::ToggleFullscreen => {
                let fullscreen = if window.fullscreen().is_some() {
                    None
                } else {
                    Some(Fullscreen::Borderless(None))
                };

                window.set_fullscreen(fullscreen);
            }
            Action::ToggleMaximize => {
                let maximized = window.is_maximized();
                window.set_maximized(!maximized);
            }
            Action::ToggleImeInput => {
                state.ime = !state.ime;
                window.set_ime_allowed(state.ime);
                if let Some(position) = state.ime.then_some(state.cursor_position).flatten() {
                    window.set_ime_cursor_area(position, PhysicalSize::new(20, 20));
                }
            }
            Action::Minimize => {
                window.set_minimized(true);
            }
            Action::NextCursor => {
                // Pick the next cursor
                state.named_idx = (state.named_idx + 1) % CURSORS.len();
                println!("Setting cursor to \"{:?}\"", CURSORS[state.named_idx]);
                window.set_cursor(Cursor::Icon(CURSORS[state.named_idx]));
            }
            Action::NextCustomCursor => {
                state.custom_idx = (state.custom_idx + 1) % self.custom_cursors.len();
                let cursor = Cursor::Custom(self.custom_cursors[state.custom_idx].clone());
                window.set_cursor(cursor);
            }
            Action::CycleCursorGrab => {
                state.cursor_grab = match state.cursor_grab {
                    CursorGrabMode::None => CursorGrabMode::Confined,
                    CursorGrabMode::Confined => CursorGrabMode::Locked,
                    CursorGrabMode::Locked => CursorGrabMode::None,
                };
                println!("Changing cursor grab mode to {:?}", state.cursor_grab);
                if let Err(err) = window.set_cursor_grab(state.cursor_grab) {
                    eprintln!("Error setting cursor grab: {err}");
                }
            }
            Action::DragWindow => {
                if let Err(err) = window.drag_window() {
                    println!("Error starting window drag: {err}");
                } else {
                    println!("Dragging window Window={:?}", window.id());
                }
            }
            Action::DragResizeWindow => {
                let position = match state.cursor_position {
                    Some(position) => position,
                    None => {
                        println!("Drag-resize requires cursor to be inside the window");
                        return;
                    }
                };

                let win_size = window.inner_size();
                let border_size = BORDER_SIZE * window.scale_factor();

                let x_direction = if position.x < border_size {
                    ResizeDirection::West
                } else if position.x > (win_size.width as f64 - border_size) {
                    ResizeDirection::East
                } else {
                    // Use arbitrary direction instead of None for simplicity.
                    ResizeDirection::SouthEast
                };

                let y_direction = if position.y < border_size {
                    ResizeDirection::North
                } else if position.y > (win_size.height as f64 - border_size) {
                    ResizeDirection::South
                } else {
                    // Use arbitrary direction instead of None for simplicity.
                    ResizeDirection::SouthEast
                };

                let direction = match (x_direction, y_direction) {
                    (ResizeDirection::West, ResizeDirection::North) => ResizeDirection::NorthWest,
                    (ResizeDirection::West, ResizeDirection::South) => ResizeDirection::SouthWest,
                    (ResizeDirection::West, _) => ResizeDirection::West,
                    (ResizeDirection::East, ResizeDirection::North) => ResizeDirection::NorthEast,
                    (ResizeDirection::East, ResizeDirection::South) => ResizeDirection::SouthEast,
                    (ResizeDirection::East, _) => ResizeDirection::East,
                    (_, ResizeDirection::South) => ResizeDirection::South,
                    (_, ResizeDirection::North) => ResizeDirection::North,
                    _ => return,
                };

                if let Err(err) = window.drag_resize_window(direction) {
                    println!("Error starting window drag-resize: {err}");
                } else {
                    println!("Drag-resizing window Window={:?}", window.id());
                }
            }
            Action::ShowWindowMenu => {
                if let Some(position) = state.cursor_position {
                    window.show_window_menu(position);
                }
            }
            Action::PrintHelp => self.print_help(),
            #[cfg(macos_platform)]
            Action::CycleOptionAsAlt => {
                let new = match window.option_as_alt() {
                    OptionAsAlt::None => OptionAsAlt::OnlyLeft,
                    OptionAsAlt::OnlyLeft => OptionAsAlt::OnlyRight,
                    OptionAsAlt::OnlyRight => OptionAsAlt::Both,
                    OptionAsAlt::Both => OptionAsAlt::None,
                };
                println!("Setting option as alt {:?}", new);
                window.set_option_as_alt(new);
            }
            #[cfg(macos_platform)]
            Action::CreateNewTab => {
                let tab_id = window.tabbing_identifier();
                if let Err(err) = self.create_window(event_loop, Some(tab_id)) {
                    eprintln!("Error creating new window: {err}");
                }
            }
        }
    }

    fn handle_window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let state = match self.windows.get_mut(&window_id) {
            Some(state) => state,
            None => return,
        };
        let window = &state.window;

        match event {
            // Resize the surface to the new size
            WindowEvent::Resized(_size) => {
                #[cfg(not(any(android_platform, ios_platform)))]
                {
                    let (width, height) =
                        match (NonZeroU32::new(_size.width), NonZeroU32::new(_size.height)) {
                            (Some(width), Some(height)) => (width, height),
                            _ => return,
                        };
                    state
                        .surface
                        .resize(width, height)
                        .expect("failed to resize inner buffer");
                }
                window.request_redraw();
            }
            WindowEvent::Focused(focused) => {
                if focused {
                    println!("Window={window_id:?} fosused");
                } else {
                    println!("Window={window_id:?} unfosused");
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                println!("Window={window_id:?} changed scale to {scale_factor}");
            }
            WindowEvent::ThemeChanged(theme) => {
                println!("Theme changed to {theme:?}");
                state.theme = theme;
                window.request_redraw();
            }
            #[cfg(not(any(android_platform, ios_platform)))]
            WindowEvent::RedrawRequested => {
                // Draw the window contents.

                if state.occluded {
                    println!("Skipping drawing occluded window={:?}", window_id);
                }

                const WHITE: u32 = 0xFFFFFFFF;
                const DARK_GRAY: u32 = 0xFF181818;

                let color = match state.theme {
                    Theme::Light => WHITE,
                    Theme::Dark => DARK_GRAY,
                };

                let mut buffer = state
                    .surface
                    .buffer_mut()
                    .expect("could not retrieve buffer");
                buffer.fill(color);
                window.pre_present_notify();
                buffer.present().expect("failed presenting to window");
            }
            #[cfg(any(android_platform, ios_platform))]
            WindowEvent::RedrawRequested => {
                println!("Drawing but without rendering...");
            }
            // Change window occlusion state.
            WindowEvent::Occluded(occluded) => {
                state.occluded = occluded;
                if !occluded {
                    window.request_redraw();
                }
            }
            WindowEvent::CloseRequested => {
                println!("Closing Window={window_id:?}");
                self.windows.remove(&window_id);
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                state.modifiers = modifiers.state();
                println!("Modifiers changed to {:?}", state.modifiers);
            }
            WindowEvent::MouseWheel { delta, .. } => match delta {
                MouseScrollDelta::LineDelta(x, y) => {
                    println!("Mouse wheel Line Delta: ({x},{y})");
                }
                MouseScrollDelta::PixelDelta(px) => {
                    println!("Mouse wheel Pixel Delta: ({},{})", px.x, px.y);
                }
            },
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        // Dispatch actions only on press.
                        state: ElementState::Pressed,
                        logical_key,
                        ..
                    },
                is_synthetic: false,
                ..
            } => {
                if let Key::Character(ch) = logical_key.as_ref() {
                    let mods = state.modifiers;
                    if let Some(action) = Self::process_key_binding(&ch.to_uppercase(), &mods) {
                        self.handle_action(event_loop, window_id, action);
                    }
                }
            }
            WindowEvent::KeyboardInput { .. } => {}
            WindowEvent::MouseInput {
                button,
                state: ElementState::Pressed,
                ..
            } => {
                if let Some(action) = Self::process_mouse_binding(button, &state.modifiers) {
                    self.handle_action(event_loop, window_id, action);
                }
            }
            WindowEvent::MouseInput { .. } => {}
            WindowEvent::CursorLeft { .. } => {
                println!("Cursor left Window={window_id:?}");
                state.cursor_position = None;
            }
            WindowEvent::CursorMoved { position, .. } => {
                println!("Moved cursor to {position:?}");
                state.cursor_position = Some(position);
                if state.ime {
                    window.set_ime_cursor_area(position, PhysicalSize::new(20, 20));
                }
            }
            WindowEvent::ActivationTokenDone { token: _token, .. } => {
                #[cfg(any(x11_platform, wayland_platform))]
                {
                    startup_notify::set_activation_token_env(_token);
                    if let Err(err) = self.create_window(event_loop, None) {
                        eprintln!("Error creating new window: {err}");
                    }
                }
            }
            WindowEvent::Ime(event) => match event {
                Ime::Enabled => println!("IME enabled for Window={window_id:?}"),
                Ime::Preedit(text, caret_pos) => {
                    println!("Preedit: {}, with caret at {:?}", text, caret_pos);
                }
                Ime::Commit(text) => {
                    println!("Commited: {}", text);
                }
                Ime::Disabled => println!("IME disabled for Window={window_id:?}"),
            },
            WindowEvent::PinchGesture { delta, .. } => {
                state.zoom += delta;
                let zoom = state.zoom;
                if delta > 0.0 {
                    println!("Zoomed in {delta:.5} (now: {zoom:.5})");
                } else {
                    println!("Zoomed out {delta:.5} (now: {zoom:.5})");
                }
            }
            WindowEvent::RotationGesture { delta, .. } => {
                state.rotated += delta;
                let rotated = state.rotated;
                if delta > 0.0 {
                    println!("Rotated counterclockwise {delta:.5} (now: {rotated:.5})");
                } else {
                    println!("Rotated clockwise {delta:.5} (now: {rotated:.5})");
                }
            }
            WindowEvent::DoubleTapGesture { .. } => {
                println!("Smart zoom");
            }
            WindowEvent::TouchpadPressure { .. }
            | WindowEvent::HoveredFileCancelled
            | WindowEvent::CursorEntered { .. }
            | WindowEvent::AxisMotion { .. }
            | WindowEvent::DroppedFile(_)
            | WindowEvent::HoveredFile(_)
            | WindowEvent::Destroyed
            | WindowEvent::Touch(_)
            | WindowEvent::Moved(_) => (),
        }
    }

    fn handle_device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        println!("Device event: {event:?}");
    }

    /// Process the key binding.
    fn process_key_binding(key: &str, mods: &ModifiersState) -> Option<Action> {
        KEY_BINDINGS.iter().find_map(|binding| {
            binding
                .is_triggered_by(&key, mods)
                .then_some(binding.action)
        })
    }

    /// Process mouse binding.
    fn process_mouse_binding(button: MouseButton, mods: &ModifiersState) -> Option<Action> {
        MOUSE_BINDINGS.iter().find_map(|binding| {
            binding
                .is_triggered_by(&button, mods)
                .then_some(binding.action)
        })
    }

    fn print_help(&self) {
        println!("Keyboard bindings:");
        for binding in KEY_BINDINGS {
            println!(
                "{}{:<10} - {} ({})",
                modifiers_to_string(binding.mods),
                binding.trigger,
                binding.action,
                binding.action.help(),
            );
        }
        println!("Mouse bindings:");
        for binding in MOUSE_BINDINGS {
            println!(
                "{}{:<10} - {} ({})",
                modifiers_to_string(binding.mods),
                mouse_button_to_string(binding.trigger),
                binding.action,
                binding.action.help(),
            );
        }
    }
}

/// Extra state on a window used in this example.
struct WindowState {
    /// The actual Winit window.
    window: Window,
    /// IME input.
    ime: bool,
    /// Render surface.
    ///
    /// NOTE: This surface must be dropped before the `Window`.
    #[cfg(not(any(android_platform, ios_platform)))]
    surface: Surface,
    /// The window theme we're drawing with.
    theme: Theme,
    /// Cursor position over the window.
    cursor_position: Option<PhysicalPosition<f64>>,
    /// Window modifiers state.
    modifiers: ModifiersState,
    /// Occlusion state of the window.
    occluded: bool,
    /// Current cursor grab mode.
    cursor_grab: CursorGrabMode,
    /// The amount of zoom into window.
    zoom: f64,
    /// The amount of rotation of the window.
    rotated: f32,
    // Cursor states.
    named_idx: usize,
    custom_idx: usize,
    cursor_hidden: bool,
}

struct Binding<T: Eq> {
    trigger: T,
    mods: ModifiersState,
    action: Action,
}

impl<T: Eq> Binding<T> {
    const fn new(trigger: T, mods: ModifiersState, action: Action) -> Self {
        Self {
            trigger,
            mods,
            action,
        }
    }

    fn is_triggered_by(&self, trigger: &T, mods: &ModifiersState) -> bool {
        &self.trigger == trigger && &self.mods == mods
    }
}

/// Helper enum describing the different kinds of actions this example can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    CloseWindow,
    ToggleCursorVisibility,
    CreateNewWindow,
    ToggleResizeIncrements,
    ToggleImeInput,
    ToggleDecorations,
    ToggleResizable,
    ToggleFullscreen,
    ToggleMaximize,
    Minimize,
    NextCursor,
    NextCustomCursor,
    CycleCursorGrab,
    PrintHelp,
    DragWindow,
    DragResizeWindow,
    ShowWindowMenu,
    #[cfg(macos_platform)]
    CycleOptionAsAlt,
    #[cfg(macos_platform)]
    CreateNewTab,
}

impl Action {
    fn help(&self) -> &'static str {
        match self {
            Action::CloseWindow => "Close window",
            Action::ToggleCursorVisibility => "Hide cursor",
            Action::CreateNewWindow => "Create new window",
            Action::ToggleImeInput => "Toggle IME input",
            Action::ToggleDecorations => "Toggle decorations",
            Action::ToggleResizable => "Toggle window resizable state",
            Action::ToggleFullscreen => "Toggle fullscreen",
            Action::ToggleMaximize => "Maximize",
            Action::Minimize => "Minimize",
            Action::ToggleResizeIncrements => "Use resize increments when resizing window",
            Action::NextCursor => "Advance the cursor to the next value",
            Action::NextCustomCursor => "Advance custom cursor to the next value",
            Action::CycleCursorGrab => "Cycle through cursor grab mode",
            Action::PrintHelp => "Print help",
            Action::DragWindow => "Start window drag",
            Action::DragResizeWindow => "Start window drag-resize",
            Action::ShowWindowMenu => "Show window menu",
            #[cfg(macos_platform)]
            Action::CycleOptionAsAlt => "Cycle option as alt mode",
            #[cfg(macos_platform)]
            Action::CreateNewTab => "Create new tab",
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self, f)
    }
}

fn decode_cursor(bytes: &[u8]) -> CustomCursorSource {
    let img = image::load_from_memory(bytes).unwrap().to_rgba8();
    let samples = img.into_flat_samples();
    let (_, w, h) = samples.extents();
    let (w, h) = (w as u16, h as u16);
    CustomCursor::from_rgba(samples.samples, w, h, w / 2, h / 2).unwrap()
}

fn load_icon(path: &Path) -> Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::open(path)
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
}

fn modifiers_to_string(mods: ModifiersState) -> String {
    let mut mods_line = String::new();
    // Always add + since it's printed as a part of the bindings.
    for (modifier, desc) in [
        (ModifiersState::SUPER, "Super+"),
        (ModifiersState::ALT, "Alt+"),
        (ModifiersState::CONTROL, "Ctrl+"),
        (ModifiersState::SHIFT, "Shift+"),
    ] {
        if !mods.contains(modifier) {
            continue;
        }

        mods_line.push_str(desc);
    }
    mods_line
}

fn mouse_button_to_string(button: MouseButton) -> &'static str {
    match button {
        MouseButton::Left => "LMB",
        MouseButton::Right => "RMB",
        MouseButton::Middle => "MMB",
        MouseButton::Back => "Back",
        MouseButton::Forward => "Forward",
        MouseButton::Other(_) => "",
    }
}

/// Cursor list to cycle through.
const CURSORS: &[CursorIcon] = &[
    CursorIcon::Default,
    CursorIcon::Crosshair,
    CursorIcon::Pointer,
    CursorIcon::Move,
    CursorIcon::Text,
    CursorIcon::Wait,
    CursorIcon::Help,
    CursorIcon::Progress,
    CursorIcon::NotAllowed,
    CursorIcon::ContextMenu,
    CursorIcon::Cell,
    CursorIcon::VerticalText,
    CursorIcon::Alias,
    CursorIcon::Copy,
    CursorIcon::NoDrop,
    CursorIcon::Grab,
    CursorIcon::Grabbing,
    CursorIcon::AllScroll,
    CursorIcon::ZoomIn,
    CursorIcon::ZoomOut,
    CursorIcon::EResize,
    CursorIcon::NResize,
    CursorIcon::NeResize,
    CursorIcon::NwResize,
    CursorIcon::SResize,
    CursorIcon::SeResize,
    CursorIcon::SwResize,
    CursorIcon::WResize,
    CursorIcon::EwResize,
    CursorIcon::NsResize,
    CursorIcon::NeswResize,
    CursorIcon::NwseResize,
    CursorIcon::ColResize,
    CursorIcon::RowResize,
];

const KEY_BINDINGS: &[Binding<&'static str>] = &[
    Binding::new("Q", ModifiersState::CONTROL, Action::CloseWindow),
    Binding::new("H", ModifiersState::CONTROL, Action::PrintHelp),
    Binding::new("F", ModifiersState::CONTROL, Action::ToggleFullscreen),
    Binding::new("D", ModifiersState::CONTROL, Action::ToggleDecorations),
    Binding::new("I", ModifiersState::CONTROL, Action::ToggleImeInput),
    Binding::new("L", ModifiersState::CONTROL, Action::CycleCursorGrab),
    Binding::new("P", ModifiersState::CONTROL, Action::ToggleResizeIncrements),
    Binding::new("R", ModifiersState::CONTROL, Action::ToggleResizable),
    // M.
    Binding::new("M", ModifiersState::CONTROL, Action::ToggleMaximize),
    Binding::new("M", ModifiersState::ALT, Action::Minimize),
    // N.
    Binding::new("N", ModifiersState::CONTROL, Action::CreateNewWindow),
    // C.
    Binding::new("C", ModifiersState::CONTROL, Action::NextCursor),
    Binding::new("C", ModifiersState::ALT, Action::NextCustomCursor),
    Binding::new("Z", ModifiersState::CONTROL, Action::ToggleCursorVisibility),
    #[cfg(macos_platform)]
    Binding::new("T", ModifiersState::SUPER, Action::CreateNewTab),
    #[cfg(macos_platform)]
    Binding::new("O", ModifiersState::CONTROL, Action::CycleOptionAsAlt),
];

const MOUSE_BINDINGS: &[Binding<MouseButton>] = &[
    Binding::new(
        MouseButton::Left,
        ModifiersState::ALT,
        Action::DragResizeWindow,
    ),
    Binding::new(
        MouseButton::Left,
        ModifiersState::CONTROL,
        Action::DragWindow,
    ),
    Binding::new(
        MouseButton::Right,
        ModifiersState::CONTROL,
        Action::ShowWindowMenu,
    ),
];
