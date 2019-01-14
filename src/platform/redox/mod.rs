use orbclient::{EventOption, Renderer};
use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex};

use {CreationError, MouseCursor, WindowAttributes, VirtualKeyCode};
use dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use window::MonitorId as RootMonitorId;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId;

fn convert_scancode(scancode: u8) -> Option<VirtualKeyCode> {
    match scancode {
        orbclient::K_A => Some(VirtualKeyCode::A),
        orbclient::K_B => Some(VirtualKeyCode::B),
        orbclient::K_C => Some(VirtualKeyCode::C),
        orbclient::K_D => Some(VirtualKeyCode::D),
        orbclient::K_E => Some(VirtualKeyCode::E),
        orbclient::K_F => Some(VirtualKeyCode::F),
        orbclient::K_G => Some(VirtualKeyCode::G),
        orbclient::K_H => Some(VirtualKeyCode::H),
        orbclient::K_I => Some(VirtualKeyCode::I),
        orbclient::K_J => Some(VirtualKeyCode::J),
        orbclient::K_K => Some(VirtualKeyCode::K),
        orbclient::K_L => Some(VirtualKeyCode::L),
        orbclient::K_M => Some(VirtualKeyCode::M),
        orbclient::K_N => Some(VirtualKeyCode::N),
        orbclient::K_O => Some(VirtualKeyCode::O),
        orbclient::K_P => Some(VirtualKeyCode::P),
        orbclient::K_Q => Some(VirtualKeyCode::Q),
        orbclient::K_R => Some(VirtualKeyCode::R),
        orbclient::K_S => Some(VirtualKeyCode::S),
        orbclient::K_T => Some(VirtualKeyCode::T),
        orbclient::K_U => Some(VirtualKeyCode::U),
        orbclient::K_V => Some(VirtualKeyCode::V),
        orbclient::K_W => Some(VirtualKeyCode::W),
        orbclient::K_X => Some(VirtualKeyCode::X),
        orbclient::K_Y => Some(VirtualKeyCode::Y),
        orbclient::K_Z => Some(VirtualKeyCode::Z),
        orbclient::K_0 => Some(VirtualKeyCode::Key0),
        orbclient::K_1 => Some(VirtualKeyCode::Key1),
        orbclient::K_2 => Some(VirtualKeyCode::Key2),
        orbclient::K_3 => Some(VirtualKeyCode::Key3),
        orbclient::K_4 => Some(VirtualKeyCode::Key4),
        orbclient::K_5 => Some(VirtualKeyCode::Key5),
        orbclient::K_6 => Some(VirtualKeyCode::Key6),
        orbclient::K_7 => Some(VirtualKeyCode::Key7),
        orbclient::K_8 => Some(VirtualKeyCode::Key8),
        orbclient::K_9 => Some(VirtualKeyCode::Key9),

        orbclient::K_TICK => Some(VirtualKeyCode::Grave),
        orbclient::K_MINUS => Some(VirtualKeyCode::Minus),
        orbclient::K_EQUALS => Some(VirtualKeyCode::Equals),
        orbclient::K_BACKSLASH => Some(VirtualKeyCode::Backslash),
        orbclient::K_BRACE_OPEN => Some(VirtualKeyCode::LBracket),
        orbclient::K_BRACE_CLOSE => Some(VirtualKeyCode::RBracket),
        orbclient::K_SEMICOLON => Some(VirtualKeyCode::Semicolon),
        orbclient::K_QUOTE => Some(VirtualKeyCode::Apostrophe),
        orbclient::K_COMMA => Some(VirtualKeyCode::Comma),
        orbclient::K_PERIOD => Some(VirtualKeyCode::Period),
        orbclient::K_SLASH => Some(VirtualKeyCode::Slash),
        orbclient::K_BKSP => Some(VirtualKeyCode::Back),
        orbclient::K_SPACE => Some(VirtualKeyCode::Space),
        orbclient::K_TAB => Some(VirtualKeyCode::Tab),
        //orbclient::K_CAPS => Some(VirtualKeyCode::CAPS),
        orbclient::K_LEFT_SHIFT => Some(VirtualKeyCode::LShift),
        orbclient::K_RIGHT_SHIFT => Some(VirtualKeyCode::RShift),
        orbclient::K_CTRL => Some(VirtualKeyCode::LControl),
        orbclient::K_ALT => Some(VirtualKeyCode::LAlt),
        orbclient::K_ENTER => Some(VirtualKeyCode::Return),
        orbclient::K_ESC => Some(VirtualKeyCode::Escape),
        orbclient::K_F1 => Some(VirtualKeyCode::F1),
        orbclient::K_F2 => Some(VirtualKeyCode::F2),
        orbclient::K_F3 => Some(VirtualKeyCode::F3),
        orbclient::K_F4 => Some(VirtualKeyCode::F4),
        orbclient::K_F5 => Some(VirtualKeyCode::F5),
        orbclient::K_F6 => Some(VirtualKeyCode::F6),
        orbclient::K_F7 => Some(VirtualKeyCode::F7),
        orbclient::K_F8 => Some(VirtualKeyCode::F8),
        orbclient::K_F9 => Some(VirtualKeyCode::F9),
        orbclient::K_F10 => Some(VirtualKeyCode::F10),
        orbclient::K_HOME => Some(VirtualKeyCode::Home),
        orbclient::K_UP => Some(VirtualKeyCode::Up),
        orbclient::K_PGUP => Some(VirtualKeyCode::PageUp),
        orbclient::K_LEFT => Some(VirtualKeyCode::Left),
        orbclient::K_RIGHT => Some(VirtualKeyCode::Right),
        orbclient::K_END => Some(VirtualKeyCode::End),
        orbclient::K_DOWN => Some(VirtualKeyCode::Down),
        orbclient::K_PGDN => Some(VirtualKeyCode::PageDown),
        orbclient::K_DEL => Some(VirtualKeyCode::Delete),
        orbclient::K_F11 => Some(VirtualKeyCode::F11),
        orbclient::K_F12 => Some(VirtualKeyCode::F12),

        _ => None
    }
}

fn element_state(pressed: bool) -> ::ElementState {
    if pressed {
        ::ElementState::Pressed
    } else {
        ::ElementState::Released
    }
}

#[derive(Default)]
struct EventState {
    lshift: bool,
    rshift: bool,
    lctrl: bool,
    rctrl: bool,
    lalt: bool,
    ralt: bool,
    llogo: bool,
    rlogo: bool,
    left: bool,
    middle: bool,
    right: bool,
}

impl EventState {
    fn key(&mut self, vk: VirtualKeyCode, pressed: bool) {
        match vk {
            VirtualKeyCode::LShift => self.lshift = pressed,
            VirtualKeyCode::RShift => self.rshift = pressed,
            VirtualKeyCode::LControl => self.lctrl = pressed,
            VirtualKeyCode::RControl => self.rctrl = pressed,
            VirtualKeyCode::LAlt => self.lalt = pressed,
            VirtualKeyCode::RAlt => self.ralt = pressed,
            VirtualKeyCode::LWin => self.llogo = pressed,
            VirtualKeyCode::RWin => self.rlogo = pressed,
            _ => ()
        }
    }

    fn mouse(&mut self, left: bool, middle: bool, right: bool) -> Option<(::MouseButton, ::ElementState)> {
        if self.left != left {
            self.left = left;
            return Some((::MouseButton::Left, element_state(self.left)));
        }

        if self.middle != middle {
            self.middle = middle;
            return Some((::MouseButton::Middle, element_state(self.middle)));
        }

        if self.right != right {
            self.right = right;
            return Some((::MouseButton::Right, element_state(self.right)));
        }

        None
    }

    fn modifiers(&self) -> ::ModifiersState {
        ::ModifiersState {
            shift: self.lshift || self.rshift,
            ctrl: self.lctrl || self.rctrl,
            alt: self.lalt || self.ralt,
            logo: self.llogo || self.rlogo,
        }
    }
}

pub struct EventsLoop(Arc<Mutex<Vec<(Arc<Mutex<orbclient::Window>>, EventState)>>>);

impl EventsLoop {
    pub fn new() -> Self {
        EventsLoop(Arc::new(Mutex::new(Vec::new())))
    }

    #[inline]
    pub fn get_available_monitors(&self) -> VecDeque<MonitorId> {
        let mut rb = VecDeque::with_capacity(1);
        rb.push_back(MonitorId);
        rb
    }

    #[inline]
    pub fn get_primary_monitor(&self) -> MonitorId {
        MonitorId
    }

    pub fn poll_events<F>(&mut self, mut callback: F)
        where F: FnMut(::Event)
    {
        for (ref arc, ref mut state) in self.0.lock().unwrap().iter_mut() {
            let mut win = arc.lock().unwrap();

            for win_event in win.events() {
                match win_event.to_option() {
                    EventOption::Key(event) => {
                        if event.scancode != 0 {
                            let vk_opt = convert_scancode(event.scancode);
                            if let Some(vk) = vk_opt {
                                state.key(vk, event.pressed);
                            }
                            callback(::Event::WindowEvent {
                                window_id: ::WindowId(WindowId),
                                event: ::WindowEvent::KeyboardInput {
                                    device_id: ::DeviceId(DeviceId),
                                    input: ::KeyboardInput {
                                        scancode: event.scancode as u32,
                                        state: element_state(event.pressed),
                                        virtual_keycode: vk_opt,
                                        modifiers: state.modifiers(),
                                    },
                                },
                            });
                        }
                        if event.character != '\0' {
                            callback(::Event::WindowEvent {
                                window_id: ::WindowId(WindowId),
                                event: ::WindowEvent::ReceivedCharacter(event.character),
                            });
                        }
                    },
                    EventOption::Mouse(event) => callback(::Event::WindowEvent {
                        window_id: ::WindowId(WindowId),
                        event: ::WindowEvent::CursorMoved {
                            device_id: ::DeviceId(DeviceId),
                            position: (event.x, event.y).into(),
                            modifiers: state.modifiers(),
                        }
                    }),
                    EventOption::Button(event) => {
                        while let Some((btn, btn_state)) = state.mouse(event.left, event.middle, event.right) {
                            callback(::Event::WindowEvent {
                                window_id: ::WindowId(WindowId),
                                event: ::WindowEvent::MouseInput {
                                    device_id: ::DeviceId(DeviceId),
                                    state: btn_state,
                                    button: btn,
                                    modifiers: state.modifiers(),
                                }
                            });
                        }
                    },
                    EventOption::Scroll(event) => callback(::Event::WindowEvent {
                        window_id: ::WindowId(WindowId),
                        event: ::WindowEvent::MouseWheel {
                            device_id: ::DeviceId(DeviceId),
                            delta: ::MouseScrollDelta::PixelDelta((event.x, event.y).into()),
                            phase: ::TouchPhase::Moved,
                            modifiers: state.modifiers(),
                        }
                    }),
                    EventOption::Quit(_event) => callback(::Event::WindowEvent {
                        window_id: ::WindowId(WindowId),
                        event: ::WindowEvent::CloseRequested,
                    }),
                    EventOption::Focus(event) => callback(::Event::WindowEvent {
                        window_id: ::WindowId(WindowId),
                        event: ::WindowEvent::Focused(event.focused),
                    }),
                    EventOption::Move(event) => callback(::Event::WindowEvent {
                        window_id: ::WindowId(WindowId),
                        event: ::WindowEvent::Moved((event.x, event.y).into()),
                    }),
                    EventOption::Resize(event) => callback(::Event::WindowEvent {
                        window_id: ::WindowId(WindowId),
                        event: ::WindowEvent::Resized((event.width, event.height).into()),
                    }),
                    _ => ()
                }
            }
        }
    }

    pub fn run_forever<F>(&mut self, mut callback: F)
        where F: FnMut(::Event) -> ::ControlFlow,
    {
        // Yeah that's a very bad implementation.
        loop {
            let mut control_flow = ::ControlFlow::Continue;
            self.poll_events(|e| {
                if let ::ControlFlow::Break = callback(e) {
                    control_flow = ::ControlFlow::Break;
                }
            });
            if let ::ControlFlow::Break = control_flow {
                break;
            }
            ::std::thread::sleep(::std::time::Duration::from_millis(5));
        }
    }

    pub fn create_proxy(&self) -> EventsLoopProxy {
        EventsLoopProxy
    }
}

#[derive(Clone)]
pub struct EventsLoopProxy;

impl EventsLoopProxy {
    pub fn wakeup(&self) -> Result<(), ::EventsLoopClosed> {
        unimplemented!()
    }
}

#[derive(Clone)]
pub struct MonitorId;

impl fmt::Debug for MonitorId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        #[derive(Debug)]
        struct MonitorId {
            name: Option<String>,
            dimensions: PhysicalSize,
            position: PhysicalPosition,
            hidpi_factor: f64,
        }

        let monitor_id_proxy = MonitorId {
            name: self.get_name(),
            dimensions: self.get_dimensions(),
            position: self.get_position(),
            hidpi_factor: self.get_hidpi_factor(),
        };

        monitor_id_proxy.fmt(f)
    }
}

impl MonitorId {
    #[inline]
    pub fn get_name(&self) -> Option<String> {
        Some("Primary".to_string())
    }

    #[inline]
    pub fn get_dimensions(&self) -> PhysicalSize {
        let size = orbclient::get_display_size().unwrap_or((0, 0));
        (size.0 as f64, size.1 as f64).into()
    }

    #[inline]
    pub fn get_position(&self) -> PhysicalPosition {
        (0, 0).into()
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f64 {
        1.0
    }
}

#[derive(Clone, Default)]
pub struct PlatformSpecificWindowBuilderAttributes;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId;

pub struct Window(Arc<Mutex<orbclient::Window>>);

impl Window {
    pub fn new(ev: &EventsLoop, attributes: WindowAttributes, _pl_attributes: PlatformSpecificWindowBuilderAttributes) -> Result<Window, CreationError> {
        let (w, h) = if let Some(dimensions) = attributes.dimensions {
            dimensions.to_physical(1.0).into()
        } else {
            (800, 600)
        };

        let mut flags = vec![orbclient::WindowFlag::Async];
        if attributes.resizable {
            flags.push(orbclient::WindowFlag::Resizable);
        }
        if attributes.transparent {
            flags.push(orbclient::WindowFlag::Transparent);
        }
        if ! attributes.decorations {
            flags.push(orbclient::WindowFlag::Borderless);
        }
        if attributes.always_on_top {
            flags.push(orbclient::WindowFlag::Front);
        }
        // TODO: More attributes like visible

        let win = orbclient::Window::new_flags(-1, -1, w, h, &attributes.title, &flags)
            .ok_or(CreationError::OsError("failed to create window".to_string()))?;

        let arc = Arc::new(Mutex::new(win));
        ev.0.lock().unwrap().push((arc.clone(), EventState::default()));

        Ok(Window(arc))
    }

    pub(crate) fn get_orbclient_window(&self) -> Arc<Mutex<orbclient::Window>> {
        self.0.clone()
    }

    #[inline]
    pub fn set_title(&self, title: &str) {
        let mut win = self.0.lock().unwrap();
        win.set_title(title);
    }

    #[inline]
    pub fn show(&self) {
        // TODO: Visibilty not supported in window server
    }

    #[inline]
    pub fn hide(&self) {
        // TODO: Visibilty not supported in window server
    }

    #[inline]
    pub fn get_position(&self) -> Option<LogicalPosition> {
        // TODO: Account for decorations
        self.get_inner_position()
    }

    #[inline]
    pub fn get_inner_position(&self) -> Option<LogicalPosition> {
        let win = self.0.lock().unwrap();
        Some((win.x(), win.y()).into())
    }

    #[inline]
    pub fn set_position(&self, position: LogicalPosition) {
        // TODO: Account for decorations
        let (x, y) = position.into();
        let mut win = self.0.lock().unwrap();
        win.set_pos(x, y);
    }

    #[inline]
    pub fn get_inner_size(&self) -> Option<LogicalSize> {
        let win = self.0.lock().unwrap();
        Some((win.width(), win.height()).into())
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<LogicalSize> {
        // TODO: Account for decorations
        self.get_inner_size()
    }

    #[inline]
    pub fn set_inner_size(&self, size: LogicalSize) {
        let (w, h) = size.into();
        let mut win = self.0.lock().unwrap();
        win.set_size(w, h);
    }

    #[inline]
    pub fn set_min_dimensions(&self, _dimensions: Option<LogicalSize>) {
        // TODO: Minimum dimensions not supported in window server
    }

    #[inline]
    pub fn set_max_dimensions(&self, _dimensions: Option<LogicalSize>) {
        // TODO: Maximum dimensions not supported in window server
    }

    #[inline]
    pub fn set_resizable(&self, _resizable: bool) {
        // TODO: Changing resizable flag not supported in window library
    }

    #[inline]
    pub fn set_cursor(&self, _cursor: MouseCursor) {
        // TODO: Setting cursor not supported in window server
    }

    #[inline]
    pub fn grab_cursor(&self, _grab: bool) -> Result<(), String> {
        // TODO: Grabbing cursor not supported in window server
        Err("Cursor grabbing is not possible on Redox".to_owned())
    }

    #[inline]
    pub fn hide_cursor(&self, _hide: bool) {
        // TODO: Hiding cursor not supported in window server
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f64 {
        1.0
    }

    #[inline]
    pub fn set_cursor_position(&self, _position: LogicalPosition) -> Result<(), String> {
        // TODO: Setting cursor position not supported in window server
        Err("Setting cursor position is not possible on Redox".to_owned())
    }

    #[inline]
    pub fn set_maximized(&self, _maximized: bool) {
        // TODO: Maximizing not supported in window library
    }

    #[inline]
    pub fn set_fullscreen(&self, _monitor: Option<RootMonitorId>) {
        // TODO: Fullscreen not supported in window library
    }

    #[inline]
    pub fn set_decorations(&self, _decorations: bool) {
        // TODO: Setting decorations after creation not supported in window library
    }

    #[inline]
    pub fn set_always_on_top(&self, _always_on_top: bool) {
        // TODO: Setting always on top after creation not supported in window library
    }

    #[inline]
    pub fn set_window_icon(&self, _icon: Option<::Icon>) {
        // TODO: Setting window icon not supported in window server
    }

    #[inline]
    pub fn set_ime_spot(&self, _logical_spot: LogicalPosition) {
        // TODO: Setting ime spot not supported in window server
    }

    #[inline]
    pub fn get_current_monitor(&self) -> RootMonitorId {
        RootMonitorId { inner: MonitorId }
    }

    #[inline]
    pub fn get_available_monitors(&self) -> VecDeque<MonitorId> {
        let mut list = VecDeque::with_capacity(1);
        list.push_back(MonitorId);
        list
    }

    #[inline]
    pub fn get_primary_monitor(&self) -> MonitorId {
        MonitorId
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        WindowId
    }
}
