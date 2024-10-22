//! Simple winit application.

use std::collections::HashMap;
use std::error::Error;
use std::fmt::Debug;
#[cfg(not(android_platform))]
use std::num::NonZeroU32;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::{fmt, mem};

use ::tracing::{error, info};
use cursor_icon::CursorIcon;
#[cfg(not(android_platform))]
use rwh_06::{DisplayHandle, HasDisplayHandle};
#[cfg(not(android_platform))]
use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use winit::error::RequestError;
use winit::event::{DeviceEvent, DeviceId, Ime, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState};
#[cfg(macos_platform)]
use winit::platform::macos::{OptionAsAlt, WindowAttributesExtMacOS, WindowExtMacOS};
#[cfg(any(x11_platform, wayland_platform))]
use winit::platform::startup_notify::{
    self, EventLoopExtStartupNotify, WindowAttributesExtStartupNotify, WindowExtStartupNotify,
};
#[cfg(web_platform)]
use winit::platform::web::{ActiveEventLoopExtWeb, CustomCursorExtWeb, WindowAttributesExtWeb};
use winit::window::{
    Cursor, CursorGrabMode, CustomCursor, CustomCursorSource, Fullscreen, Icon, ResizeDirection,
    Theme, Window, WindowAttributes, WindowId,
};

#[path = "util/tracing.rs"]
mod tracing;

/// The amount of points to around the window for drag resize direction calculations.
const BORDER_SIZE: f64 = 20.;

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(web_platform)]
    console_error_panic_hook::set_once();

    tracing::init();

    let event_loop = EventLoop::new()?;
    let (sender, receiver) = mpsc::channel();

    // Wire the user event from another thread.
    #[cfg(not(web_platform))]
    {
        let event_loop_proxy = event_loop.create_proxy();
        let sender = sender.clone();
        std::thread::spawn(move || {
            // Wake up the `event_loop` once every second and dispatch a custom event
            // from a different thread.
            info!("Starting to send user event every second");
            loop {
                let _ = sender.send(Action::Message);
                event_loop_proxy.wake_up();
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        });
    }

    let app = Application::new(&event_loop, receiver, sender);
    Ok(event_loop.run_app(app)?)
}

/// Application state and event handling.
struct Application {
    /// Trigger actions through proxy wake up.
    receiver: Receiver<Action>,
    sender: Sender<Action>,
    /// Custom cursors assets.
    custom_cursors: Result<Vec<CustomCursor>, RequestError>,
    /// Application icon.
    icon: Icon,
    windows: HashMap<WindowId, WindowState>,
    /// Drawing context.
    ///
    /// With OpenGL it could be EGLDisplay.
    #[cfg(not(android_platform))]
    context: Option<Context<DisplayHandle<'static>>>,
}

impl Application {
    fn new(event_loop: &EventLoop, receiver: Receiver<Action>, sender: Sender<Action>) -> Self {
        // SAFETY: we drop the context right before the event loop is stopped, thus making it safe.
        #[cfg(not(android_platform))]
        let context = Some(
            Context::new(unsafe {
                std::mem::transmute::<DisplayHandle<'_>, DisplayHandle<'static>>(
                    event_loop.display_handle().unwrap(),
                )
            })
            .unwrap(),
        );

        // You'll have to choose an icon size at your own discretion. On X11, the desired size
        // varies by WM, and on Windows, you still have to account for screen scaling. Here
        // we use 32px, since it seems to work well enough in most cases. Be careful about
        // going too high, or you'll be bitten by the low-quality downscaling built into the
        // WM.
        let icon = load_icon(include_bytes!("data/icon.png"));

        info!("Loading cursor assets");
        let custom_cursors = [
            event_loop.create_custom_cursor(decode_cursor(include_bytes!("data/cross.png"))),
            event_loop.create_custom_cursor(decode_cursor(include_bytes!("data/cross2.png"))),
            event_loop.create_custom_cursor(decode_cursor(include_bytes!("data/gradient.png"))),
        ]
        .into_iter()
        .collect();

        Self {
            receiver,
            sender,
            #[cfg(not(android_platform))]
            context,
            custom_cursors,
            icon,
            windows: Default::default(),
        }
    }

    fn create_window(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        _tab_id: Option<String>,
    ) -> Result<WindowId, Box<dyn Error>> {
        // TODO read-out activation token.

        #[allow(unused_mut)]
        let mut window_attributes = WindowAttributes::default()
            .with_title("Winit window")
            .with_transparent(true)
            .with_window_icon(Some(self.icon.clone()));

        #[cfg(any(x11_platform, wayland_platform))]
        if let Some(token) = event_loop.read_token_from_env() {
            startup_notify::reset_activation_token_env();
            info!("Using token {:?} to activate a window", token);
            window_attributes = window_attributes.with_activation_token(token);
        }

        #[cfg(macos_platform)]
        if let Some(tab_id) = _tab_id {
            window_attributes = window_attributes.with_tabbing_identifier(&tab_id);
        }

        #[cfg(web_platform)]
        {
            window_attributes = window_attributes.with_append(true);
        }

        let window = event_loop.create_window(window_attributes)?;

        #[cfg(ios_platform)]
        {
            use winit::platform::ios::WindowExtIOS;
            window.recognize_doubletap_gesture(true);
            window.recognize_pinch_gesture(true);
            window.recognize_rotation_gesture(true);
            window.recognize_pan_gesture(true, 2, 2);
        }

        let window_state = WindowState::new(self, window)?;
        let window_id = window_state.window.id();
        info!("Created new window with id={window_id:?}");
        self.windows.insert(window_id, window_state);
        Ok(window_id)
    }

    fn handle_action_from_proxy(&mut self, _event_loop: &dyn ActiveEventLoop, action: Action) {
        match action {
            #[cfg(web_platform)]
            Action::DumpMonitors => self.dump_monitors(_event_loop),
            Action::Message => {
                info!("User wake up");
            },
            _ => unreachable!("Tried to execute invalid action without `WindowId`"),
        }
    }

    fn handle_action_with_window(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        action: Action,
    ) {
        // let cursor_position = self.cursor_position;
        let window = self.windows.get_mut(&window_id).unwrap();
        info!("Executing action: {action:?}");
        match action {
            Action::CloseWindow => {
                let _ = self.windows.remove(&window_id);
            },
            Action::CreateNewWindow => {
                #[cfg(any(x11_platform, wayland_platform))]
                if let Err(err) = window.window.request_activation_token() {
                    info!("Failed to get activation token: {err}");
                } else {
                    return;
                }

                if let Err(err) = self.create_window(event_loop, None) {
                    error!("Error creating new window: {err}");
                }
            },
            Action::ToggleResizeIncrements => window.toggle_resize_increments(),
            Action::ToggleCursorVisibility => window.toggle_cursor_visibility(),
            Action::ToggleResizable => window.toggle_resizable(),
            Action::ToggleDecorations => window.toggle_decorations(),
            Action::ToggleFullscreen => window.toggle_fullscreen(),
            Action::ToggleMaximize => window.toggle_maximize(),
            Action::ToggleImeInput => window.toggle_ime(),
            Action::Minimize => window.minimize(),
            Action::NextCursor => window.next_cursor(),
            Action::NextCustomCursor => {
                if let Err(err) = self.custom_cursors.as_ref().map(|c| window.next_custom_cursor(c))
                {
                    error!("Error creating custom cursor: {err}");
                }
            },
            #[cfg(web_platform)]
            Action::UrlCustomCursor => {
                if let Err(err) = window.url_custom_cursor(event_loop) {
                    error!("Error creating custom cursor from URL: {err}");
                }
            },
            #[cfg(web_platform)]
            Action::AnimationCustomCursor => {
                if let Err(err) = self
                    .custom_cursors
                    .as_ref()
                    .map(|c| window.animation_custom_cursor(event_loop, c))
                {
                    error!("Error creating animated custom cursor: {err}");
                }
            },
            Action::CycleCursorGrab => window.cycle_cursor_grab(),
            Action::DragWindow => window.drag_window(),
            Action::DragResizeWindow => window.drag_resize_window(),
            Action::ShowWindowMenu => window.show_menu(),
            Action::PrintHelp => self.print_help(),
            #[cfg(macos_platform)]
            Action::CycleOptionAsAlt => window.cycle_option_as_alt(),
            Action::SetTheme(theme) => {
                window.window.set_theme(theme);
                // Get the resulting current theme to draw with
                let actual_theme = theme.or_else(|| window.window.theme()).unwrap_or(Theme::Dark);
                window.set_draw_theme(actual_theme);
            },
            #[cfg(macos_platform)]
            Action::CreateNewTab => {
                let tab_id = window.window.tabbing_identifier();
                if let Err(err) = self.create_window(event_loop, Some(tab_id)) {
                    error!("Error creating new window: {err}");
                }
            },
            Action::RequestResize => window.swap_dimensions(),
            #[cfg(web_platform)]
            Action::DumpMonitors => {
                let future = event_loop.request_detailed_monitor_permission();
                let proxy = event_loop.create_proxy();
                let sender = self.sender.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Err(error) = future.await {
                        error!("{error}")
                    }

                    let _ = sender.send(Action::DumpMonitors);
                    proxy.wake_up();
                });
            },
            #[cfg(not(web_platform))]
            Action::DumpMonitors => self.dump_monitors(event_loop),
            Action::Message => {
                self.sender.send(Action::Message).unwrap();
                event_loop.create_proxy().wake_up();
            },
        }
    }

    fn dump_monitors(&self, event_loop: &dyn ActiveEventLoop) {
        info!("Monitors information");
        let primary_monitor = event_loop.primary_monitor();
        for monitor in event_loop.available_monitors() {
            let intro = if primary_monitor.as_ref() == Some(&monitor) {
                "Primary monitor"
            } else {
                "Monitor"
            };

            if let Some(name) = monitor.name() {
                info!("{intro}: {name}");
            } else {
                info!("{intro}: [no name]");
            }

            if let Some(current_mode) = monitor.current_video_mode() {
                let PhysicalSize { width, height } = current_mode.size();
                let bits =
                    current_mode.bit_depth().map(|bits| format!("x{bits}")).unwrap_or_default();
                let m_hz = current_mode
                    .refresh_rate_millihertz()
                    .map(|m_hz| format!(" @ {}.{} Hz", m_hz.get() / 1000, m_hz.get() % 1000))
                    .unwrap_or_default();
                info!("  {width}x{height}{bits}{m_hz}");
            }

            if let Some(PhysicalPosition { x, y }) = monitor.position() {
                info!("  Position: {x},{y}");
            }

            info!("  Scale factor: {}", monitor.scale_factor());

            info!("  Available modes (width x height x bit-depth):");
            for mode in monitor.video_modes() {
                let PhysicalSize { width, height } = mode.size();
                let bits = mode.bit_depth().map(|bits| format!("x{bits}")).unwrap_or_default();
                let m_hz = mode
                    .refresh_rate_millihertz()
                    .map(|m_hz| format!(" @ {}.{} Hz", m_hz.get() / 1000, m_hz.get() % 1000))
                    .unwrap_or_default();
                info!("    {width}x{height}{bits}{m_hz}");
            }
        }
    }

    /// Process the key binding.
    fn process_key_binding(key: &str, mods: &ModifiersState) -> Option<Action> {
        KEY_BINDINGS
            .iter()
            .find_map(|binding| binding.is_triggered_by(&key, mods).then_some(binding.action))
    }

    /// Process mouse binding.
    fn process_mouse_binding(button: MouseButton, mods: &ModifiersState) -> Option<Action> {
        MOUSE_BINDINGS
            .iter()
            .find_map(|binding| binding.is_triggered_by(&button, mods).then_some(binding.action))
    }

    fn print_help(&self) {
        info!("Keyboard bindings:");
        for binding in KEY_BINDINGS {
            info!(
                "{}{:<10} - {} ({})",
                modifiers_to_string(binding.mods),
                binding.trigger,
                binding.action,
                binding.action.help(),
            );
        }
        info!("Mouse bindings:");
        for binding in MOUSE_BINDINGS {
            info!(
                "{}{:<10} - {} ({})",
                modifiers_to_string(binding.mods),
                mouse_button_to_string(binding.trigger),
                binding.action,
                binding.action.help(),
            );
        }
    }
}

impl ApplicationHandler for Application {
    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        while let Ok(action) = self.receiver.try_recv() {
            self.handle_action_from_proxy(event_loop, action)
        }
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let window = match self.windows.get_mut(&window_id) {
            Some(window) => window,
            None => return,
        };

        match event {
            WindowEvent::SurfaceResized(size) => {
                window.resize(size);
            },
            WindowEvent::Focused(focused) => {
                if focused {
                    info!("Window={window_id:?} focused");
                } else {
                    info!("Window={window_id:?} unfocused");
                }
            },
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                info!("Window={window_id:?} changed scale to {scale_factor}");
            },
            WindowEvent::ThemeChanged(theme) => {
                info!("Theme changed to {theme:?}");
                window.set_draw_theme(theme);
            },
            WindowEvent::RedrawRequested => {
                if let Err(err) = window.draw() {
                    error!("Error drawing window: {err}");
                }
            },
            WindowEvent::Occluded(occluded) => {
                window.set_occluded(occluded);
            },
            WindowEvent::CloseRequested => {
                info!("Closing Window={window_id:?}");
                self.windows.remove(&window_id);
            },
            WindowEvent::ModifiersChanged(modifiers) => {
                window.modifiers = modifiers.state();
                info!("Modifiers changed to {:?}", window.modifiers);
            },
            WindowEvent::MouseWheel { delta, .. } => match delta {
                MouseScrollDelta::LineDelta(x, y) => {
                    info!("Mouse wheel Line Delta: ({x},{y})");
                },
                MouseScrollDelta::PixelDelta(px) => {
                    info!("Mouse wheel Pixel Delta: ({},{})", px.x, px.y);
                },
            },
            WindowEvent::KeyboardInput { event, is_synthetic: false, .. } => {
                let mods = window.modifiers;

                // Dispatch actions only on press.
                if event.state.is_pressed() {
                    let action = if let Key::Character(ch) = event.logical_key.as_ref() {
                        Self::process_key_binding(&ch.to_uppercase(), &mods)
                    } else {
                        None
                    };

                    if let Some(action) = action {
                        self.handle_action_with_window(event_loop, window_id, action);
                    }
                }
            },
            WindowEvent::PointerButton { button, state, .. } => {
                info!("Pointer button {button:?} {state:?}");
                let mods = window.modifiers;
                if let Some(action) = state
                    .is_pressed()
                    .then(|| Self::process_mouse_binding(button.mouse_button(), &mods))
                    .flatten()
                {
                    self.handle_action_with_window(event_loop, window_id, action);
                }
            },
            WindowEvent::PointerLeft { .. } => {
                info!("Pointer left Window={window_id:?}");
                window.cursor_left();
            },
            WindowEvent::PointerMoved { position, .. } => {
                info!("Moved pointer to {position:?}");
                window.cursor_moved(position);
            },
            WindowEvent::ActivationTokenDone { token: _token, .. } => {
                #[cfg(any(x11_platform, wayland_platform))]
                {
                    startup_notify::set_activation_token_env(_token);
                    if let Err(err) = self.create_window(event_loop, None) {
                        error!("Error creating new window: {err}");
                    }
                }
            },
            WindowEvent::Ime(event) => match event {
                Ime::Enabled => info!("IME enabled for Window={window_id:?}"),
                Ime::Preedit(text, caret_pos) => {
                    info!("Preedit: {}, with caret at {:?}", text, caret_pos);
                },
                Ime::Commit(text) => {
                    info!("Committed: {}", text);
                },
                Ime::Disabled => info!("IME disabled for Window={window_id:?}"),
            },
            WindowEvent::PinchGesture { delta, .. } => {
                window.zoom += delta;
                let zoom = window.zoom;
                if delta > 0.0 {
                    info!("Zoomed in {delta:.5} (now: {zoom:.5})");
                } else {
                    info!("Zoomed out {delta:.5} (now: {zoom:.5})");
                }
            },
            WindowEvent::RotationGesture { delta, .. } => {
                window.rotated += delta;
                let rotated = window.rotated;
                if delta > 0.0 {
                    info!("Rotated counterclockwise {delta:.5} (now: {rotated:.5})");
                } else {
                    info!("Rotated clockwise {delta:.5} (now: {rotated:.5})");
                }
            },
            WindowEvent::PanGesture { delta, phase, .. } => {
                window.panned.x += delta.x;
                window.panned.y += delta.y;
                info!("Panned ({delta:?})) (now: {:?}), {phase:?}", window.panned);
            },
            WindowEvent::DoubleTapGesture { .. } => {
                info!("Smart zoom");
            },
            WindowEvent::TouchpadPressure { .. }
            | WindowEvent::HoveredFileCancelled
            | WindowEvent::KeyboardInput { .. }
            | WindowEvent::PointerEntered { .. }
            | WindowEvent::DroppedFile(_)
            | WindowEvent::HoveredFile(_)
            | WindowEvent::Destroyed
            | WindowEvent::Moved(_) => (),
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &dyn ActiveEventLoop,
        device_id: Option<DeviceId>,
        event: DeviceEvent,
    ) {
        info!("Device {device_id:?} event: {event:?}");
    }

    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        info!("Ready to create surfaces");
        self.dump_monitors(event_loop);

        // Create initial window.
        self.create_window(event_loop, None).expect("failed to create initial window");

        self.print_help();
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        if self.windows.is_empty() {
            info!("No windows left, exiting...");
            event_loop.exit();
        }
    }

    #[cfg(not(android_platform))]
    fn exiting(&mut self, _event_loop: &dyn ActiveEventLoop) {
        // We must drop the context here.
        self.context = None;
    }
}

/// State of the window.
struct WindowState {
    /// IME input.
    ime: bool,
    /// Render surface.
    ///
    /// NOTE: This surface must be dropped before the `Window`.
    #[cfg(not(android_platform))]
    surface: Surface<DisplayHandle<'static>, Arc<dyn Window>>,
    /// The actual winit Window.
    window: Arc<dyn Window>,
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
    /// The amount of pan of the window.
    panned: PhysicalPosition<f32>,

    #[cfg(macos_platform)]
    option_as_alt: OptionAsAlt,

    // Cursor states.
    named_idx: usize,
    custom_idx: usize,
    cursor_hidden: bool,
}

impl WindowState {
    fn new(app: &Application, window: Box<dyn Window>) -> Result<Self, Box<dyn Error>> {
        let window: Arc<dyn Window> = Arc::from(window);

        // SAFETY: the surface is dropped before the `window` which provided it with handle, thus
        // it doesn't outlive it.
        #[cfg(not(android_platform))]
        let surface = Surface::new(app.context.as_ref().unwrap(), Arc::clone(&window))?;

        let theme = window.theme().unwrap_or(Theme::Dark);
        info!("Theme: {theme:?}");
        let named_idx = 0;
        window.set_cursor(CURSORS[named_idx].into());

        // Allow IME out of the box.
        let ime = true;
        window.set_ime_allowed(ime);

        let size = window.surface_size();
        let mut state = Self {
            #[cfg(macos_platform)]
            option_as_alt: window.option_as_alt(),
            custom_idx: app.custom_cursors.as_ref().map(Vec::len).unwrap_or(1) - 1,
            cursor_grab: CursorGrabMode::None,
            named_idx,
            #[cfg(not(android_platform))]
            surface,
            window,
            theme,
            ime,
            cursor_position: Default::default(),
            cursor_hidden: Default::default(),
            modifiers: Default::default(),
            occluded: Default::default(),
            rotated: Default::default(),
            panned: Default::default(),
            zoom: Default::default(),
        };

        state.resize(size);
        Ok(state)
    }

    pub fn toggle_ime(&mut self) {
        self.ime = !self.ime;
        self.window.set_ime_allowed(self.ime);
        if let Some(position) = self.ime.then_some(self.cursor_position).flatten() {
            self.window.set_ime_cursor_area(position.into(), PhysicalSize::new(20, 20).into());
        }
    }

    pub fn minimize(&mut self) {
        self.window.set_minimized(true);
    }

    pub fn cursor_moved(&mut self, position: PhysicalPosition<f64>) {
        self.cursor_position = Some(position);
        if self.ime {
            self.window.set_ime_cursor_area(position.into(), PhysicalSize::new(20, 20).into());
        }
    }

    pub fn cursor_left(&mut self) {
        self.cursor_position = None;
    }

    /// Toggle maximized.
    fn toggle_maximize(&self) {
        let maximized = self.window.is_maximized();
        self.window.set_maximized(!maximized);
    }

    /// Toggle window decorations.
    fn toggle_decorations(&self) {
        let decorated = self.window.is_decorated();
        self.window.set_decorations(!decorated);
    }

    /// Toggle window resizable state.
    fn toggle_resizable(&self) {
        let resizable = self.window.is_resizable();
        self.window.set_resizable(!resizable);
    }

    /// Toggle cursor visibility
    fn toggle_cursor_visibility(&mut self) {
        self.cursor_hidden = !self.cursor_hidden;
        self.window.set_cursor_visible(!self.cursor_hidden);
    }

    /// Toggle resize increments on a window.
    fn toggle_resize_increments(&mut self) {
        let new_increments = match self.window.surface_resize_increments() {
            Some(_) => None,
            None => Some(LogicalSize::new(25.0, 25.0).into()),
        };
        info!("Had increments: {}", new_increments.is_none());
        self.window.set_surface_resize_increments(new_increments);
    }

    /// Toggle fullscreen.
    fn toggle_fullscreen(&self) {
        let fullscreen = if self.window.fullscreen().is_some() {
            None
        } else {
            Some(Fullscreen::Borderless(None))
        };

        self.window.set_fullscreen(fullscreen);
    }

    /// Cycle through the grab modes ignoring errors.
    fn cycle_cursor_grab(&mut self) {
        self.cursor_grab = match self.cursor_grab {
            CursorGrabMode::None => CursorGrabMode::Confined,
            CursorGrabMode::Confined => CursorGrabMode::Locked,
            CursorGrabMode::Locked => CursorGrabMode::None,
        };
        info!("Changing cursor grab mode to {:?}", self.cursor_grab);
        if let Err(err) = self.window.set_cursor_grab(self.cursor_grab) {
            error!("Error setting cursor grab: {err}");
        }
    }

    #[cfg(macos_platform)]
    fn cycle_option_as_alt(&mut self) {
        self.option_as_alt = match self.option_as_alt {
            OptionAsAlt::None => OptionAsAlt::OnlyLeft,
            OptionAsAlt::OnlyLeft => OptionAsAlt::OnlyRight,
            OptionAsAlt::OnlyRight => OptionAsAlt::Both,
            OptionAsAlt::Both => OptionAsAlt::None,
        };
        info!("Setting option as alt {:?}", self.option_as_alt);
        self.window.set_option_as_alt(self.option_as_alt);
    }

    /// Swap the window dimensions with `request_surface_size`.
    fn swap_dimensions(&mut self) {
        let old_surface_size = self.window.surface_size();
        let mut surface_size = old_surface_size;

        mem::swap(&mut surface_size.width, &mut surface_size.height);
        info!("Requesting resize from {old_surface_size:?} to {surface_size:?}");

        if let Some(new_surface_size) = self.window.request_surface_size(surface_size.into()) {
            if old_surface_size == new_surface_size {
                info!("Inner size change got ignored");
            } else {
                self.resize(new_surface_size);
            }
        } else {
            info!("Requesting surface size is asynchronous");
        }
    }

    /// Pick the next cursor.
    fn next_cursor(&mut self) {
        self.named_idx = (self.named_idx + 1) % CURSORS.len();
        info!("Setting cursor to \"{:?}\"", CURSORS[self.named_idx]);
        self.window.set_cursor(Cursor::Icon(CURSORS[self.named_idx]));
    }

    /// Pick the next custom cursor.
    fn next_custom_cursor(&mut self, custom_cursors: &[CustomCursor]) {
        self.custom_idx = (self.custom_idx + 1) % custom_cursors.len();
        let cursor = Cursor::Custom(custom_cursors[self.custom_idx].clone());
        self.window.set_cursor(cursor);
    }

    /// Custom cursor from an URL.
    #[cfg(web_platform)]
    fn url_custom_cursor(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
    ) -> Result<(), Box<dyn Error>> {
        let cursor = event_loop.create_custom_cursor(url_custom_cursor())?;

        self.window.set_cursor(cursor.into());

        Ok(())
    }

    /// Custom cursor from a URL.
    #[cfg(web_platform)]
    fn animation_custom_cursor(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        custom_cursors: &[CustomCursor],
    ) -> Result<(), Box<dyn Error>> {
        use std::time::Duration;

        let cursors = vec![
            custom_cursors[0].clone(),
            custom_cursors[1].clone(),
            event_loop.create_custom_cursor(url_custom_cursor())?,
        ];
        let cursor = CustomCursor::from_animation(Duration::from_secs(3), cursors).unwrap();
        let cursor = event_loop.create_custom_cursor(cursor)?;

        self.window.set_cursor(cursor.into());

        Ok(())
    }

    /// Resize the surface to the new size.
    fn resize(&mut self, size: PhysicalSize<u32>) {
        info!("Surface resized to {size:?}");
        #[cfg(not(android_platform))]
        {
            let (width, height) = match (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
            {
                (Some(width), Some(height)) => (width, height),
                _ => return,
            };
            self.surface.resize(width, height).expect("failed to resize inner buffer");
        }
        self.window.request_redraw();
    }

    /// Change the theme that things are drawn in.
    fn set_draw_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.window.request_redraw();
    }

    /// Show window menu.
    fn show_menu(&self) {
        if let Some(position) = self.cursor_position {
            self.window.show_window_menu(position.into());
        }
    }

    /// Drag the window.
    fn drag_window(&self) {
        if let Err(err) = self.window.drag_window() {
            info!("Error starting window drag: {err}");
        } else {
            info!("Dragging window Window={:?}", self.window.id());
        }
    }

    /// Drag-resize the window.
    fn drag_resize_window(&self) {
        let position = match self.cursor_position {
            Some(position) => position,
            None => {
                info!("Drag-resize requires cursor to be inside the window");
                return;
            },
        };

        let win_size = self.window.surface_size();
        let border_size = BORDER_SIZE * self.window.scale_factor();

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

        if let Err(err) = self.window.drag_resize_window(direction) {
            info!("Error starting window drag-resize: {err}");
        } else {
            info!("Drag-resizing window Window={:?}", self.window.id());
        }
    }

    /// Change window occlusion state.
    fn set_occluded(&mut self, occluded: bool) {
        self.occluded = occluded;
        if !occluded {
            self.window.request_redraw();
        }
    }

    /// Draw the window contents.
    #[cfg(not(android_platform))]
    fn draw(&mut self) -> Result<(), Box<dyn Error>> {
        if self.occluded {
            info!("Skipping drawing occluded window={:?}", self.window.id());
            return Ok(());
        }

        const WHITE: u32 = 0xffffffff;
        const DARK_GRAY: u32 = 0xff181818;

        let color = match self.theme {
            Theme::Light => WHITE,
            Theme::Dark => DARK_GRAY,
        };

        let mut buffer = self.surface.buffer_mut()?;
        buffer.fill(color);
        self.window.pre_present_notify();
        buffer.present()?;
        Ok(())
    }

    #[cfg(android_platform)]
    fn draw(&mut self) -> Result<(), Box<dyn Error>> {
        info!("Drawing but without rendering...");
        Ok(())
    }
}

struct Binding<T: Eq> {
    trigger: T,
    mods: ModifiersState,
    action: Action,
}

impl<T: Eq> Binding<T> {
    const fn new(trigger: T, mods: ModifiersState, action: Action) -> Self {
        Self { trigger, mods, action }
    }

    fn is_triggered_by(&self, trigger: &T, mods: &ModifiersState) -> bool {
        &self.trigger == trigger && &self.mods == mods
    }
}

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
    #[cfg(web_platform)]
    UrlCustomCursor,
    #[cfg(web_platform)]
    AnimationCustomCursor,
    CycleCursorGrab,
    PrintHelp,
    DragWindow,
    DragResizeWindow,
    ShowWindowMenu,
    #[cfg(macos_platform)]
    CycleOptionAsAlt,
    SetTheme(Option<Theme>),
    #[cfg(macos_platform)]
    CreateNewTab,
    RequestResize,
    DumpMonitors,
    Message,
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
            #[cfg(web_platform)]
            Action::UrlCustomCursor => "Custom cursor from an URL",
            #[cfg(web_platform)]
            Action::AnimationCustomCursor => "Custom cursor from an animation",
            Action::CycleCursorGrab => "Cycle through cursor grab mode",
            Action::PrintHelp => "Print help",
            Action::DragWindow => "Start window drag",
            Action::DragResizeWindow => "Start window drag-resize",
            Action::ShowWindowMenu => "Show window menu",
            #[cfg(macos_platform)]
            Action::CycleOptionAsAlt => "Cycle option as alt mode",
            Action::SetTheme(None) => "Change to the system theme",
            Action::SetTheme(Some(Theme::Light)) => "Change to a light theme",
            Action::SetTheme(Some(Theme::Dark)) => "Change to a dark theme",
            #[cfg(macos_platform)]
            Action::CreateNewTab => "Create new tab",
            Action::RequestResize => "Request a resize",
            #[cfg(not(web_platform))]
            Action::DumpMonitors => "Dump monitor information",
            #[cfg(web_platform)]
            Action::DumpMonitors => {
                "Request permission to query detailed monitor information and dump monitor \
                 information"
            },
            Action::Message => "Prints a message through a user wake up",
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

#[cfg(web_platform)]
fn url_custom_cursor() -> CustomCursorSource {
    use std::sync::atomic::{AtomicU64, Ordering};

    static URL_COUNTER: AtomicU64 = AtomicU64::new(0);

    CustomCursor::from_url(
        format!("https://picsum.photos/128?random={}", URL_COUNTER.fetch_add(1, Ordering::Relaxed)),
        64,
        64,
    )
}

fn load_icon(bytes: &[u8]) -> Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::load_from_memory(bytes).unwrap().into_rgba8();
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
    Binding::new("R", ModifiersState::ALT, Action::RequestResize),
    // M.
    Binding::new("M", ModifiersState::CONTROL.union(ModifiersState::ALT), Action::DumpMonitors),
    Binding::new("M", ModifiersState::CONTROL, Action::ToggleMaximize),
    Binding::new("M", ModifiersState::ALT, Action::Minimize),
    // N.
    Binding::new("N", ModifiersState::CONTROL, Action::CreateNewWindow),
    // C.
    Binding::new("C", ModifiersState::CONTROL, Action::NextCursor),
    Binding::new("C", ModifiersState::ALT, Action::NextCustomCursor),
    #[cfg(web_platform)]
    Binding::new(
        "C",
        ModifiersState::CONTROL.union(ModifiersState::SHIFT),
        Action::UrlCustomCursor,
    ),
    #[cfg(web_platform)]
    Binding::new(
        "C",
        ModifiersState::ALT.union(ModifiersState::SHIFT),
        Action::AnimationCustomCursor,
    ),
    Binding::new("Z", ModifiersState::CONTROL, Action::ToggleCursorVisibility),
    // K.
    Binding::new("K", ModifiersState::empty(), Action::SetTheme(None)),
    Binding::new("K", ModifiersState::SUPER, Action::SetTheme(Some(Theme::Light))),
    Binding::new("K", ModifiersState::CONTROL, Action::SetTheme(Some(Theme::Dark))),
    #[cfg(macos_platform)]
    Binding::new("T", ModifiersState::SUPER, Action::CreateNewTab),
    #[cfg(macos_platform)]
    Binding::new("O", ModifiersState::CONTROL, Action::CycleOptionAsAlt),
    Binding::new("S", ModifiersState::CONTROL, Action::Message),
];

const MOUSE_BINDINGS: &[Binding<MouseButton>] = &[
    Binding::new(MouseButton::Left, ModifiersState::ALT, Action::DragResizeWindow),
    Binding::new(MouseButton::Left, ModifiersState::CONTROL, Action::DragWindow),
    Binding::new(MouseButton::Right, ModifiersState::CONTROL, Action::ShowWindowMenu),
];
