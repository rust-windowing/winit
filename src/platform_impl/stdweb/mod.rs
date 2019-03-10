use dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use event::{DeviceEvent, DeviceId as RootDI, ElementState, Event, KeyboardInput, ModifiersState, MouseButton, ScanCode, StartCause, VirtualKeyCode, WindowEvent};
use event_loop::{ControlFlow, EventLoopWindowTarget as RootELW, EventLoopClosed};
use icon::Icon;
use monitor::{MonitorHandle as RootMH};
use window::{CreationError, MouseCursor, WindowId as RootWI, WindowAttributes};
use stdweb::{
    JsSerialize,
    traits::*,
    unstable::TryInto,
    web::{
        document,
        event::*,
        html_element::CanvasElement,
    },
};
use std::cell::{RefCell, RefMut};
use std::collections::VecDeque;
use std::collections::vec_deque::IntoIter as VecDequeIter;
use std::marker::PhantomData;
use std::rc::Rc;

// TODO: dpi
// TODO: close events (stdweb PR required)
// TODO: pointer locking (stdweb PR required)
// TODO: mouse wheel events (stdweb PR required)
// TODO: key event: .which() (stdweb PR)
// TODO: should there be a maximization / fullscreen API?

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(i32);

impl DeviceId {
    pub unsafe fn dummy() -> Self {
        DeviceId(0)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MonitorHandle;

impl MonitorHandle {
    pub fn get_hidpi_factor(&self) -> f64 {
        // TODO
        1.0
    }

    pub fn get_position(&self) -> PhysicalPosition {
        unimplemented!();
    }

    pub fn get_dimensions(&self) -> PhysicalSize {
        unimplemented!();
    }

    pub fn get_name(&self) -> Option<String> {
        unimplemented!();
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId;

impl WindowId {
    pub unsafe fn dummy() -> WindowId {
        WindowId
    }
}

pub struct Window {
    canvas: CanvasElement,
}

impl Window {
    pub fn new<T>(target: &EventLoopWindowTarget<T>, attr: WindowAttributes,
                  _: PlatformSpecificWindowBuilderAttributes) -> Result<Self, CreationError> {
        let element = document()
            .create_element("canvas")
            .map_err(|_| CreationError::OsError("Failed to create canvas element".to_owned()))?;
        let canvas: CanvasElement = element.try_into()
            .map_err(|_| CreationError::OsError("Failed to create canvas element".to_owned()))?;
        document().body()
            .ok_or_else(|| CreationError::OsError("Failed to find body node".to_owned()))?
            .append_child(&canvas);
        let window = Window { canvas };
        if let Some(dimensions) = attr.dimensions {
            window.set_inner_size(dimensions);
        } else {
            window.set_inner_size(LogicalSize {
                width: 1024.0,
                height: 768.0,
            })
        }
        // TODO: most of these are no-op, but should they stay here just in case?
        window.set_min_dimensions(attr.min_dimensions);
        window.set_max_dimensions(attr.max_dimensions);
        window.set_resizable(attr.resizable);
        window.set_title(&attr.title);
        window.set_maximized(attr.maximized);
        if attr.visible {
            window.show();
        } else {
            window.hide();
        }
        //window.set_transparent(attr.transparent);
        window.set_decorations(attr.decorations);
        window.set_always_on_top(attr.always_on_top);
        window.set_window_icon(attr.window_icon);
        target.register_window(&window);
        Ok(window)
    }

    pub fn set_title(&self, title: &str) {
        document().set_title(title);
    }

    pub fn show(&self) {
        // Intentionally a no-op
    }

    pub fn hide(&self) {
        // Intentionally a no-op
    }

    pub fn request_redraw(&self) {
        // TODO: what does this mean? If it's a 'present'-style call then it's not necessary
    }

    pub fn get_position(&self) -> Option<LogicalPosition> {
        let bounds = self.canvas.get_bounding_client_rect();
        Some(LogicalPosition {
            x: bounds.get_x(),
            y: bounds.get_y(),
        })
    }

    pub fn get_inner_position(&self) -> Option<LogicalPosition> {
        self.get_inner_position()
    }

    pub fn set_position(&self, position: LogicalPosition) {
        // TODO: use CSS?
    }

    #[inline]
    pub fn get_inner_size(&self) -> Option<LogicalSize> {
        Some(LogicalSize {
            width: self.canvas.width() as f64,
            height: self.canvas.height() as f64
        })
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<LogicalSize> {
        Some(LogicalSize {
            width: self.canvas.width() as f64,
            height: self.canvas.height() as f64
        })
    }

    #[inline]
    pub fn set_inner_size(&self, size: LogicalSize) {
        self.canvas.set_width(size.width as u32);
        self.canvas.set_height(size.height as u32);
    }

    #[inline]
    pub fn set_min_dimensions(&self, _dimensions: Option<LogicalSize>) {
        // Intentionally a no-op: users can't resize canvas elements
    }

    #[inline]
    pub fn set_max_dimensions(&self, _dimensions: Option<LogicalSize>) {
        // Intentionally a no-op: users can't resize canvas elements
    }

    #[inline]
    pub fn set_resizable(&self, _resizable: bool) {
        // Intentionally a no-op: users can't resize canvas elements
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f64 {
        // TODO
        1.0
    }

    #[inline]
    pub fn set_cursor(&self, cursor: MouseCursor) {
        let text = match cursor {
            MouseCursor::Default => "auto",
            MouseCursor::Crosshair => "crosshair",
            MouseCursor::Hand => "pointer",
            MouseCursor::Arrow => "default",
            MouseCursor::Move => "move",
            MouseCursor::Text => "text",
            MouseCursor::Wait => "wait",
            MouseCursor::Help => "help",
            MouseCursor::Progress => "progress",

            MouseCursor::NotAllowed => "not-allowed",
            MouseCursor::ContextMenu => "context-menu",
            MouseCursor::Cell => "cell",
            MouseCursor::VerticalText => "vertical-text",
            MouseCursor::Alias => "alias",
            MouseCursor::Copy => "copy",
            MouseCursor::NoDrop => "no-drop",
            MouseCursor::Grab => "grab",
            MouseCursor::Grabbing => "grabbing",
            MouseCursor::AllScroll => "all-scroll",
            MouseCursor::ZoomIn => "zoom-in",
            MouseCursor::ZoomOut => "zoom-out",

            MouseCursor::EResize => "e-resize",
            MouseCursor::NResize => "n-resize",
            MouseCursor::NeResize => "ne-resize",
            MouseCursor::NwResize => "nw-resize",
            MouseCursor::SResize => "s-resize",
            MouseCursor::SeResize => "se-resize",
            MouseCursor::SwResize => "sw-resize",
            MouseCursor::WResize => "w-resize",
            MouseCursor::EwResize => "ew-resize",
            MouseCursor::NsResize => "ns-resize",
            MouseCursor::NeswResize => "nesw-resize",
            MouseCursor::NwseResize => "nwse-resize",
            MouseCursor::ColResize => "col-resize",
            MouseCursor::RowResize => "row-resize",
        };
        self.canvas.set_attribute("cursor", text)
            .expect("Setting the cursor on the canvas");
    }

    #[inline]
    pub fn set_cursor_position(&self, position: LogicalPosition) -> Result<(), String> {
        // TODO: pointer capture
        Ok(())
    }

    #[inline]
    pub fn grab_cursor(&self, grab: bool) -> Result<(), String> {
        // TODO: pointer capture
        Ok(())
    }

    #[inline]
    pub fn hide_cursor(&self, hide: bool) {
        self.canvas.set_attribute("cursor", "none")
            .expect("Setting the cursor on the canvas");
    }

    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        // TODO: should there be a maximization / fullscreen API?
    }

    #[inline]
    pub fn set_fullscreen(&self, monitor: Option<RootMH>) {
        // TODO: should there be a maximization / fullscreen API?
    }

    #[inline]
    pub fn set_decorations(&self, _decorations: bool) {
        // Intentionally a no-op, no canvas decorations
    }

    #[inline]
    pub fn set_always_on_top(&self, _always_on_top: bool) {
        // Intentionally a no-op, no window ordering
    }

    #[inline]
    pub fn set_window_icon(&self, window_icon: Option<Icon>) {
        // TODO: should this set the favicon?
    }

    #[inline]
    pub fn set_ime_spot(&self, position: LogicalPosition) {
        // TODO: what is this?
    }

    #[inline]
    pub fn get_current_monitor(&self) -> RootMH {
        RootMH {
            inner: MonitorHandle
        }
    }

    #[inline]
    pub fn get_available_monitors(&self) -> VecDequeIter<MonitorHandle> {
        VecDeque::new().into_iter()
    }

    #[inline]
    pub fn get_primary_monitor(&self) -> MonitorHandle {
        MonitorHandle
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        // TODO ?
        unsafe { WindowId::dummy() }
    }
}

pub struct EventLoop<T: 'static> {
    elw: RootELW<T>,
}

#[derive(Clone)]
struct EventLoopData<T> {
    events: VecDeque<Event<T>>,
    control: ControlFlow,
}

pub struct EventLoopWindowTarget<T: 'static> {
    data: Rc<RefCell<EventLoopData<T>>>,
}

impl<T> EventLoop<T> {
    pub fn new() -> Self {
        EventLoop {
            elw: RootELW {
                p: EventLoopWindowTarget {
                    data: Rc::new(RefCell::new(EventLoopData {
                        events: VecDeque::new(),
                        control: ControlFlow::Poll
                    }))
                },
                _marker: PhantomData
            }
        }
    }

    pub fn get_available_monitors(&self) -> VecDequeIter<MonitorHandle> {
        VecDeque::new().into_iter()
    }

    pub fn get_primary_monitor(&self) -> MonitorHandle {
        MonitorHandle
    }

    pub fn run<F>(mut self, event_handler: F)
        where F: 'static + FnMut(Event<T>, &RootELW<T>, &mut ControlFlow)
    {
        // TODO: Create event handlers for the JS events
        // TODO: how to handle request redraw?
        // TODO: onclose (stdweb PR)
        // TODO: file dropping, PathBuf isn't useful for web

        let document = &document();
        self.elw.p.add_event(document, |mut data, event: BlurEvent| {
        });
        self.elw.p.add_event(document, |mut data, event: FocusEvent| {
        });

        stdweb::event_loop(); // TODO: this is only necessary for stdweb emscripten, should it be here?
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            data: self.elw.p.data.clone()
        }
    }

    pub fn window_target(&self) -> &RootELW<T> {
        &self.elw
    }
}

impl<T> EventLoopWindowTarget<T> {
    fn register_window(&self, other: &Window) {
        let canvas = &other.canvas;
        
        self.add_event(canvas, |mut data, event: KeyDownEvent| {
            let key = event.key();
            let mut characters = key.chars();
            let first = characters.next();
            let second = characters.next();
            if let (Some(key), None) = (first, second) {
                data.events.push_back(Event::WindowEvent {
                    window_id: RootWI(WindowId),
                    event: WindowEvent::ReceivedCharacter(key)
                });
            }
            data.events.push_back(Event::WindowEvent {
                window_id: RootWI(WindowId),
                event: WindowEvent::KeyboardInput {
                    // TODO: is there a way to get keyboard device?
                    device_id: RootDI(unsafe { DeviceId::dummy() }),
                    input: KeyboardInput {
                        scancode: scancode(&event),
                        state: ElementState::Pressed,
                        virtual_keycode: button_mapping(&event),
                        modifiers: keyboard_modifiers_state(&event),
                    }
                }
            });
        });
        self.add_event(canvas, |mut data, event: KeyUpEvent| {
            data.events.push_back(Event::WindowEvent {
                window_id: RootWI(WindowId),
                event: WindowEvent::KeyboardInput {
                    // TODO: is there a way to get keyboard device?
                    device_id: RootDI(unsafe { DeviceId::dummy() }),
                    input: KeyboardInput {
                        scancode: scancode(&event),
                        state: ElementState::Released,
                        virtual_keycode: button_mapping(&event),
                        modifiers: keyboard_modifiers_state(&event),
                    }
                }
            });
        });
        self.add_event(canvas, |mut data, event: PointerOutEvent| {
            data.events.push_back(Event::WindowEvent {
                window_id: RootWI(WindowId),
                event: WindowEvent::CursorLeft {
                    device_id: RootDI(DeviceId(event.pointer_id()))
                }
            });
        });
        self.add_event(canvas, |mut data, event: PointerOverEvent| {
            data.events.push_back(Event::WindowEvent {
                window_id: RootWI(WindowId),
                event: WindowEvent::CursorEntered {
                    device_id: RootDI(DeviceId(event.pointer_id()))
                }
            });
        });
        self.add_event(canvas, |mut data, event: PointerMoveEvent| {
            data.events.push_back(Event::WindowEvent {
                window_id: RootWI(WindowId),
                event: WindowEvent::CursorMoved {
                    device_id: RootDI(DeviceId(event.pointer_id())),
                    position: LogicalPosition {
                        x: event.offset_x(),
                        y: event.offset_y()
                    },
                    modifiers: mouse_modifiers_state(&event)
                }
            });
        });
        self.add_event(canvas, |mut data, event: PointerUpEvent| {
            data.events.push_back(Event::WindowEvent {
                window_id: RootWI(WindowId),
                event: WindowEvent::MouseInput {
                    device_id: RootDI(DeviceId(event.pointer_id())),
                    state: ElementState::Pressed,
                    button: mouse_button(&event),
                    modifiers: mouse_modifiers_state(&event)
                }
            });
        });
        self.add_event(canvas, |mut data, event: PointerDownEvent| {
            data.events.push_back(Event::WindowEvent {
                window_id: RootWI(WindowId),
                event: WindowEvent::MouseInput {
                    device_id: RootDI(DeviceId(event.pointer_id())),
                    state: ElementState::Released,
                    button: mouse_button(&event),
                    modifiers: mouse_modifiers_state(&event)
                }
            });
        });
    }


    fn add_event<E, F>(&self, target: &impl IEventTarget, mut handler: F) 
            where E: ConcreteEvent, F: FnMut(RefMut<EventLoopData<T>>, E) + 'static {
        let data = self.data.clone();

        target.add_event_listener(move |event: E| {
            event.prevent_default();
            event.stop_propagation();
            event.cancel_bubble();

            handler(data.borrow_mut(), event);
        });
    }
}

fn mouse_modifiers_state(event: &impl IMouseEvent) -> ModifiersState {
    ModifiersState {
        shift: event.shift_key(),
        ctrl: event.ctrl_key(),
        alt: event.alt_key(),
        logo: event.meta_key(),
    }
}

fn mouse_button(event: &impl IMouseEvent) -> MouseButton {
    match event.button() {
        stdweb::web::event::MouseButton::Left => MouseButton::Left,
        stdweb::web::event::MouseButton::Right => MouseButton::Right,
        stdweb::web::event::MouseButton::Wheel => MouseButton::Middle,
        stdweb::web::event::MouseButton::Button4 => MouseButton::Other(0),
        stdweb::web::event::MouseButton::Button5 => MouseButton::Other(1),
    }
}

fn keyboard_modifiers_state(event: &impl IKeyboardEvent) -> ModifiersState {
    ModifiersState {
        shift: event.shift_key(),
        ctrl: event.ctrl_key(),
        alt: event.alt_key(),
        logo: event.meta_key(),
    }
}

fn scancode<T: JsSerialize>(event: &T) -> ScanCode {
    let which = js! ( return @{event}.which(); );
    which.try_into().expect("The which value should be a number")
}

fn button_mapping(event: &impl IKeyboardEvent) -> Option<VirtualKeyCode> {
    Some(match &event.code()[..] {
        "Digit1" => VirtualKeyCode::Key1,
        "Digit2" => VirtualKeyCode::Key2,
        "Digit3" => VirtualKeyCode::Key3,
        "Digit4" => VirtualKeyCode::Key4,
        "Digit5" => VirtualKeyCode::Key5,
        "Digit6" => VirtualKeyCode::Key6,
        "Digit7" => VirtualKeyCode::Key7,
        "Digit8" => VirtualKeyCode::Key8,
        "Digit9" => VirtualKeyCode::Key9,
        "Digit0" => VirtualKeyCode::Key0,
        "KeyA" => VirtualKeyCode::A,
        "KeyB" => VirtualKeyCode::B,
        "KeyC" => VirtualKeyCode::C,
        "KeyD" => VirtualKeyCode::D,
        "KeyE" => VirtualKeyCode::E,
        "KeyF" => VirtualKeyCode::F,
        "KeyG" => VirtualKeyCode::G,
        "KeyH" => VirtualKeyCode::H,
        "KeyI" => VirtualKeyCode::I,
        "KeyJ" => VirtualKeyCode::J,
        "KeyK" => VirtualKeyCode::K,
        "KeyL" => VirtualKeyCode::L,
        "KeyM" => VirtualKeyCode::M,
        "KeyN" => VirtualKeyCode::N,
        "KeyO" => VirtualKeyCode::O,
        "KeyP" => VirtualKeyCode::P,
        "KeyQ" => VirtualKeyCode::Q,
        "KeyR" => VirtualKeyCode::R,
        "KeyS" => VirtualKeyCode::S,
        "KeyT" => VirtualKeyCode::T,
        "KeyU" => VirtualKeyCode::U,
        "KeyV" => VirtualKeyCode::V,
        "KeyW" => VirtualKeyCode::W,
        "KeyX" => VirtualKeyCode::X,
        "KeyY" => VirtualKeyCode::Y,
        "KeyZ" => VirtualKeyCode::Z,
        "Escape" => VirtualKeyCode::Escape,
        "F1" => VirtualKeyCode::F1,
        "F2" => VirtualKeyCode::F2,
        "F3" => VirtualKeyCode::F3,
        "F4" => VirtualKeyCode::F4,
        "F5" => VirtualKeyCode::F5,
        "F6" => VirtualKeyCode::F6,
        "F7" => VirtualKeyCode::F7,
        "F8" => VirtualKeyCode::F8,
        "F9" => VirtualKeyCode::F9,
        "F10" => VirtualKeyCode::F10,
        "F11" => VirtualKeyCode::F11,
        "F12" => VirtualKeyCode::F12,
        "F13" => VirtualKeyCode::F13,
        "F14" => VirtualKeyCode::F14,
        "F15" => VirtualKeyCode::F15,
        "F16" => VirtualKeyCode::F16,
        "F17" => VirtualKeyCode::F17,
        "F18" => VirtualKeyCode::F18,
        "F19" => VirtualKeyCode::F19,
        "F20" => VirtualKeyCode::F20,
        "F21" => VirtualKeyCode::F21,
        "F22" => VirtualKeyCode::F22,
        "F23" => VirtualKeyCode::F23,
        "F24" => VirtualKeyCode::F24,
        "PrintScreen" => VirtualKeyCode::Snapshot,
        "ScrollLock" => VirtualKeyCode::Scroll,
        "Pause" => VirtualKeyCode::Pause,
        "Insert" => VirtualKeyCode::Insert,
        "Home" => VirtualKeyCode::Home,
        "Delete" => VirtualKeyCode::Delete,
        "End" => VirtualKeyCode::End,
        "PageDown" => VirtualKeyCode::PageDown,
        "PageUp" => VirtualKeyCode::PageUp,
        "ArrowLeft" => VirtualKeyCode::Left,
        "ArrowUp" => VirtualKeyCode::Up,
        "ArrowRight" => VirtualKeyCode::Right,
        "ArrowDown" => VirtualKeyCode::Down,
        "Backspace" => VirtualKeyCode::Back,
        "Enter" => VirtualKeyCode::Return,
        "Space" => VirtualKeyCode::Space,
        "Compose" => VirtualKeyCode::Compose,
        "Caret" => VirtualKeyCode::Caret,
        "NumLock" => VirtualKeyCode::Numlock,
        "Numpad0" => VirtualKeyCode::Numpad0,
        "Numpad1" => VirtualKeyCode::Numpad1,
        "Numpad2" => VirtualKeyCode::Numpad2,
        "Numpad3" => VirtualKeyCode::Numpad3,
        "Numpad4" => VirtualKeyCode::Numpad4,
        "Numpad5" => VirtualKeyCode::Numpad5,
        "Numpad6" => VirtualKeyCode::Numpad6,
        "Numpad7" => VirtualKeyCode::Numpad7,
        "Numpad8" => VirtualKeyCode::Numpad8,
        "Numpad9" => VirtualKeyCode::Numpad9,
        "AbntC1" => VirtualKeyCode::AbntC1,
        "AbntC2" => VirtualKeyCode::AbntC2,
        "NumpadAdd" => VirtualKeyCode::Add,
        "Quote" => VirtualKeyCode::Apostrophe,
        "Apps" => VirtualKeyCode::Apps,
        "At" => VirtualKeyCode::At,
        "Ax" => VirtualKeyCode::Ax,
        "Backslash" => VirtualKeyCode::Backslash,
        "Calculator" => VirtualKeyCode::Calculator,
        "Capital" => VirtualKeyCode::Capital,
        "Semicolon" => VirtualKeyCode::Semicolon,
        "Comma" => VirtualKeyCode::Comma,
        "Convert" => VirtualKeyCode::Convert,
        "NumpadDecimal" => VirtualKeyCode::Decimal,
        "NumpadDivide" => VirtualKeyCode::Divide,
        "Equal" => VirtualKeyCode::Equals,
        "Backquote" => VirtualKeyCode::Grave,
        "Kana" => VirtualKeyCode::Kana,
        "Kanji" => VirtualKeyCode::Kanji,
        "AltLeft" => VirtualKeyCode::LAlt,
        "BracketLeft" => VirtualKeyCode::LBracket,
        "ControlLeft" => VirtualKeyCode::LControl,
        "ShiftLeft" => VirtualKeyCode::LShift,
        "MetaLeft" => VirtualKeyCode::LWin,
        "Mail" => VirtualKeyCode::Mail,
        "MediaSelect" => VirtualKeyCode::MediaSelect,
        "MediaStop" => VirtualKeyCode::MediaStop,
        "Minus" => VirtualKeyCode::Minus,
        "NumpadMultiply" => VirtualKeyCode::Multiply,
        "Mute" => VirtualKeyCode::Mute,
        "LaunchMyComputer" => VirtualKeyCode::MyComputer,
        "NavigateForward" => VirtualKeyCode::NavigateForward,
        "NavigateBackward" => VirtualKeyCode::NavigateBackward,
        "NextTrack" => VirtualKeyCode::NextTrack,
        "NoConvert" => VirtualKeyCode::NoConvert,
        "NumpadComma" => VirtualKeyCode::NumpadComma,
        "NumpadEnter" => VirtualKeyCode::NumpadEnter,
        "NumpadEquals" => VirtualKeyCode::NumpadEquals,
        "OEM102" => VirtualKeyCode::OEM102,
        "Period" => VirtualKeyCode::Period,
        "PlayPause" => VirtualKeyCode::PlayPause,
        "Power" => VirtualKeyCode::Power,
        "PrevTrack" => VirtualKeyCode::PrevTrack,
        "AltRight" => VirtualKeyCode::RAlt,
        "BracketRight" => VirtualKeyCode::RBracket,
        "ControlRight" => VirtualKeyCode::RControl,
        "ShiftRight" => VirtualKeyCode::RShift,
        "MetaRight" => VirtualKeyCode::RWin,
        "Slash" => VirtualKeyCode::Slash,
        "Sleep" => VirtualKeyCode::Sleep,
        "Stop" => VirtualKeyCode::Stop,
        "NumpadSubtract" => VirtualKeyCode::Subtract,
        "Sysrq" => VirtualKeyCode::Sysrq,
        "Tab" => VirtualKeyCode::Tab,
        "Underline" => VirtualKeyCode::Underline,
        "Unlabeled" => VirtualKeyCode::Unlabeled,
        "AudioVolumeDown" => VirtualKeyCode::VolumeDown,
        "AudioVolumeUp" => VirtualKeyCode::VolumeUp,
        "Wake" => VirtualKeyCode::Wake,
        "WebBack" => VirtualKeyCode::WebBack,
        "WebFavorites" => VirtualKeyCode::WebFavorites,
        "WebForward" => VirtualKeyCode::WebForward,
        "WebHome" => VirtualKeyCode::WebHome,
        "WebRefresh" => VirtualKeyCode::WebRefresh,
        "WebSearch" => VirtualKeyCode::WebSearch,
        "WebStop" => VirtualKeyCode::WebStop,
        "Yen" => VirtualKeyCode::Yen,
        _ => return None
    })
}

#[derive(Clone)]
pub struct EventLoopProxy<T> {
    data: Rc<RefCell<EventLoopData<T>>>
}

impl<T> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed> {
        self.data.borrow_mut().events.push_back(Event::UserEvent(event));
        Ok(())
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlatformSpecificWindowBuilderAttributes;

