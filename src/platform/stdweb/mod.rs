#![cfg(all(feature = "stdweb", target_arch = "wasm32"))]

use std::{mem, ptr, str};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::os::raw::{c_char, c_void, c_double, c_ulong, c_int};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Arc};

use stdweb::{
    Value,
    unstable::TryInto,
    web::{
        document, window,
        event::{BlurEvent, ConcreteEvent, FocusEvent, GamepadConnectedEvent,
                GamepadDisconnectedEvent, IKeyboardEvent, IMouseEvent, IGamepadEvent,
                KeyboardLocation, KeyDownEvent, KeyUpEvent, MouseButton, 
                MouseDownEvent, MouseMoveEvent, MouseOverEvent, MouseOutEvent, MouseUpEvent},
        html_element::CanvasElement, 
        IEventTarget, IParentNode, IWindowOrWorker,
    }
};

use dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use window::MonitorId as RootMonitorId;

fn get_hidpi_factor() -> f64 {
    window().device_pixel_ratio()
}

#[derive(Clone, Default)]
pub struct PlatformSpecificWindowBuilderAttributes;

unsafe impl Send for PlatformSpecificWindowBuilderAttributes {}
unsafe impl Sync for PlatformSpecificWindowBuilderAttributes {}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId;

#[derive(Clone, Default)]
pub struct PlatformSpecificHeadlessBuilderAttributes;

#[derive(Debug, Clone)]
pub struct MonitorId;

impl MonitorId {
    #[inline]
    pub fn get_name(&self) -> Option<String> {
        Some("Canvas".to_owned())
    }

    #[inline]
    pub fn get_position(&self) -> PhysicalPosition {
        unimplemented!()
    }

    #[inline]
    pub fn get_dimensions(&self) -> PhysicalSize {
        (0, 0).into()
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f64 {
        get_hidpi_factor()
    }
}


thread_local!(static MAIN_LOOP_CALLBACK: RefCell<*mut c_void> = RefCell::new(ptr::null_mut()));

pub fn set_main_loop_callback<F>(callback : F) where F : FnMut() {
    MAIN_LOOP_CALLBACK.with(|log| {
        *log.borrow_mut() = &callback as *const _ as *mut c_void;
    });

   window().request_animation_frame(wrapper::<F>);

    fn wrapper<F>(_: f64) where F : FnMut() {
        MAIN_LOOP_CALLBACK.with(|z| {
            let closure = *z.borrow_mut() as *mut F;
            (*closure)();
        });
        window().request_animation_frame(wrapper::<F>);
    }
}


#[derive(Clone)]
pub struct EventsLoopProxy;

impl EventsLoopProxy {
    pub fn wakeup(&self) -> Result<(), ::EventsLoopClosed> {
        unimplemented!()
    }
}

pub struct EventsLoop {
    window: Mutex<Option<Arc<Window2>>>,
    interrupted: AtomicBool,
}

impl EventsLoop {
    pub fn new() -> EventsLoop {
        EventsLoop {
            window: Mutex::new(None),
            interrupted: AtomicBool::new(false),
        }
    }

    #[inline]
    pub fn interrupt(&self) {
        self.interrupted.store(true, Ordering::Relaxed);
    }

    #[inline]
    pub fn create_proxy(&self) -> EventsLoopProxy {
        unimplemented!()
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

    pub fn poll_events<F>(&self, mut callback: F)
        where F: FnMut(::Event)
    {
        let ref mut window = *self.window.lock().unwrap();
        if let &mut Some(ref mut window) = window {
            while let Some(event) = window.events.lock().unwrap().pop_front() {
                callback(event)
            }
        }
    }

    pub fn run_forever<F>(&self, mut callback: F)
        where F: FnMut(::Event) -> ::ControlFlow
    {
        self.interrupted.store(false, Ordering::Relaxed);

        // TODO: handle control flow

        set_main_loop_callback(|| {
            self.poll_events(|e| { callback(e); });
            ::std::thread::sleep(::std::time::Duration::from_millis(5));
            if self.interrupted.load(Ordering::Relaxed) {
                //TODO: interrupt the loop
            }
        });
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(usize);

pub struct Window2 {
    cursor_grabbed: Mutex<bool>,
    cursor_hidden: Mutex<bool>,
    is_fullscreen: bool,
    events: Box<Mutex<VecDeque<::Event>>>,
}

pub struct Window {
    window: Arc<Window2>,
}

fn show_mouse() {
    //TODO: show_mouse
    // Hide mouse hasn't show mouse equivalent.
    // There is a pull request on emscripten that hasn't been merged #4616
    // that contains:
    //
    // var styleSheet = document.styleSheets[0];
    // var rules = styleSheet.cssRules;
    // for (var i = 0; i < rules.length; i++) {
    //   if (rules[i].cssText.substr(0, 6) == 'canvas') {
    //     styleSheet.deleteRule(i);
    //     i--;
    //   }
    // }
    // styleSheet.insertRule('canvas.emscripten { border: none; cursor: auto; }', 0);
    js! {
        var styleSheet = document.styleSheets[0];
        var rules = styleSheet.cssRules;
        for (var i = 0; i < rules.length; i++) {
            if (rules[i].cssText.substr(0, 6) == "canvas") {
                styleSheet.deleteRule(i); i--;
            }
        }
        styleSheet.insertRule("canvas.emscripten { border: none; cursor: auto; }", 0);
    }
}

fn mouse_modifier_state(event: &impl IMouseEvent) -> ::ModifiersState {
    ::ModifiersState {
        shift: event.shitft_key(),
        ctrl: event.ctrl_key(),
        alt: event.alt_key(),
        logo: event.meta_key()
    }
}

fn mouse_move_callback(event: MouseMoveEvent, queue: &Mutex<VecDeque<::Event>>) {
    let modifiers = mouse_modifier_state(&event);
    let dpi_factor = get_hidpi_factor();
    let position = LogicalPosition::from_physical(
        (event.client_x(), event.client_y()),
        dpi_factor,
    );
    queue.lock().unwrap().push_back(::Event::WindowEvent {
        window_id: ::WindowId(WindowId(0)),
        event: ::WindowEvent::CursorMoved {
            device_id: ::DeviceId(DeviceId),
            position,
            modifiers,
        }
    });
    queue.lock().unwrap().push_back(::Event::DeviceEvent {
        device_id: ::DeviceId(DeviceId),
        event: ::DeviceEvent::MouseMotion {
            delta: (event.movement_x(), event.movement_y()),
        }
    });
}

fn mouse_button_callback(event: impl IMouseEvent, queue: &Mutex<VecDeque<::Event>>, state: ::ElementState) {
    let modifiers = mouse_modifier_state(&event);
    let button = match event.button() {
        MouseButton::Left => ::MouseButton::Left,
        MouseButton::Middle => ::MouseButton::Middle,
        MouseButton::Right => ::MouseButton::Right,
        MouseButton::Button4 => ::MouseButton::Other(4),
        MouseButton::Button5 => ::MouseButton::Other(5),
    };
    queue.lock().unwrap().push_back(::Event::WindowEvent {
        window_id: ::WindowId(WindowId(0)),
        event: ::WindowEvent::MouseInput {
            device_id: ::DeviceId(DeviceId),
            state,
            button,
            modifiers,
        }
    });
}

fn mouse_down_callback(event: MouseDownEvent, queue: &Mutex<VecDeque<::Event>>) {
    mouse_button_callback(event, queue, ::ElementState::Pressed);
}

fn mouse_up_callback(event: MouseUpEvent, queue: &Mutex<VecDeque<::Event>>) {
    mouse_button_callback(event, queue, ::ElementState::Released);
}

fn keyboard_modifier_state(event: &impl IKeyboardEvent) -> ::ModifiersState {
    ::ModifiersState {
        shift: event.shitft_key(),
        ctrl: event.ctrl_key(),
        alt: event.alt_key(),
        logo: event.meta_key()
    }
}

fn keyboard_callback(event: KeyDownEvent, queue: &Mutex<VecDeque<::Event>>, state: ::ElementState) {
    let modifiers = keyboard_modifier_state(&event);

    queue.lock().unwrap().push_back(::Event::WindowEvent {
        window_id: ::WindowId(WindowId(0)),
        event: ::WindowEvent::KeyboardInput {
            device_id: ::DeviceId(DeviceId),
            input: ::KeyboardInput {
                scancode: key_translate(event.key()) as u32,
                state,
                virtual_keycode: key_translate_virt(event.key(), event.location()),
                modifiers,
            },
        },
    });
}

fn keyboard_down_callback(event: KeyDownEvent, queue: &Mutex<VecDeque<::Event>>) {
    keyboard_callback(event, queue, ElementState::Pressed);
}

fn keyboard_up_callback(event: KeyUpEvent, queue: &Mutex<VecDeque<::Event>>) {
    keyboard_callback(event, queue, ElementState::Released);
}

/*
TODO: touch events aren't implemented in stdweb yet
extern fn touch_callback(
    event_type: c_int,
    event: *const ffi::EmscriptenTouchEvent,
    event_queue: *mut c_void) -> ffi::EM_BOOL
{
    unsafe {
        let queue: &Mutex<VecDeque<::Event>> = mem::transmute(event_queue);

        let phase = match event_type {
            ffi::EMSCRIPTEN_EVENT_TOUCHSTART => ::TouchPhase::Started,
            ffi::EMSCRIPTEN_EVENT_TOUCHEND => ::TouchPhase::Ended,
            ffi::EMSCRIPTEN_EVENT_TOUCHMOVE => ::TouchPhase::Moved,
            ffi::EMSCRIPTEN_EVENT_TOUCHCANCEL => ::TouchPhase::Cancelled,
            _ => return ffi::EM_FALSE,
        };

        for touch in 0..(*event).numTouches as usize {
            let touch = (*event).touches[touch];
            if touch.isChanged == ffi::EM_TRUE {
                let dpi_factor = get_hidpi_factor();
                let location = LogicalPosition::from_physical(
                    (touch.canvasX as f64, touch.canvasY as f64),
                    dpi_factor,
                );
                queue.lock().unwrap().push_back(::Event::WindowEvent {
                    window_id: ::WindowId(WindowId(0)),
                    event: ::WindowEvent::Touch(::Touch {
                        device_id: ::DeviceId(DeviceId),
                        phase,
                        id: touch.identifier as u64,
                        location,
                    }),
                });
            }
        }
    }
    ffi::EM_FALSE
}
*/

/*
TODO: fullscreen isn't implemented in stdweb yet
// In case of fullscreen window this method will request fullscreen on change
#[allow(non_snake_case)]
unsafe extern "C" fn fullscreen_callback(
    _eventType: c_int,
    _fullscreenChangeEvent: *const ffi::EmscriptenFullscreenChangeEvent,
    _userData: *mut c_void) -> ffi::EM_BOOL
{
    ffi::emscripten_request_fullscreen(ptr::null(), ffi::EM_TRUE);
    ffi::EM_FALSE
}
*/

/*
TODO: pointerlock change isn't implemented in stdweb yet
// In case of pointer grabbed this method will request pointer lock on change
#[allow(non_snake_case)]
unsafe extern "C" fn pointerlockchange_callback(
    _eventType: c_int,
    _pointerlockChangeEvent: *const ffi::EmscriptenPointerlockChangeEvent,
    _userData: *mut c_void) -> ffi::EM_BOOL
{
    ffi::emscripten_request_pointerlock(ptr::null(), ffi::EM_TRUE);
    ffi::EM_FALSE
}
*/


impl Window {
    pub fn new(events_loop: &EventsLoop, attribs: ::WindowAttributes,
               _pl_attribs: PlatformSpecificWindowBuilderAttributes)
        -> Result<Window, ::CreationError>
    {
        if events_loop.window.lock().unwrap().is_some() {
            return Err(::CreationError::OsError("Cannot create another window".to_owned()));
        }

        let w = Window2 {
            cursor_grabbed: Default::default(),
            cursor_hidden: Default::default(),
            events: Default::default(),
            is_fullscreen: attribs.fullscreen.is_some(),
        };

        let window = Window {
            window: Arc::new(w),
        };

        let doc = document();
        
        // TODO: set up more event callbacks

        doc.add_event_listener(|event: MouseMoveEvent| mouse_move_callback(event, &*window.window.events));
        doc.add_event_listener(|event: MouseDownEvent| mouse_down_callback(event, &*window.window.events));
        doc.add_event_listener(|event: MouseUpEvent| mouse_up_callback(event, &*window.window.events));
        doc.add_event_listener(|event: KeyDownEvent| keyboard_down_callback(event, &*window.window.events));
        doc.add_event_listener(|event: KeyUpEvent| keyboard_up_callback(event, &*window.window.events));
        // TODO: touchstart, touchend, touchmove, touchcancel


        if attribs.fullscreen.is_some() {
            // TODO: request fullscreen, fullscreen callback
        } else if let Some(size) = attribs.dimensions {
            window.set_inner_size(size);
        }

        *events_loop.window.lock().unwrap() = Some(window.window.clone());
        Ok(window)
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        WindowId(0)
    }

    #[inline]
    pub fn set_title(&self, _title: &str) {
    }

    #[inline]
    pub fn get_position(&self) -> Option<LogicalPosition> {
        Some((0, 0).into())
    }

    #[inline]
    pub fn get_inner_position(&self) -> Option<LogicalPosition> {
        Some((0, 0).into())
    }

    #[inline]
    pub fn set_position(&self, _: LogicalPosition) {
    }

    #[inline]
    pub fn get_inner_size(&self) -> Option<LogicalSize> {
        match document.query_selector("#canvas").unwrap()?.try_into() {
            Ok(Some(elem)) => match elem.try_into::<CanvasElement>() {
                Ok(canvas) => {
                    let dpi_factor = self.get_hidpi_factor();
                    let logical = LogicalSize::from_physical((canvas.width(), canvas.height()), dpi_factor);
                    Some(logical)
                }
                Err(_) => None
            }
            Err(_) => None
        }
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<LogicalSize> {
        self.get_inner_size()
    }

    #[inline]
    pub fn set_inner_size(&self, size: LogicalSize) {
        let dpi_factor = self.get_hidpi_factor();
        let physical = PhysicalSize::from_logical(size, dpi_factor);
        let (width, height): (u32, u32) = physical.into();
        match document.query_selector("#canvas").unwrap()?.try_into() {
            Ok(Some(elem)) => match elem.try_into::<CanvasElement>() {
                Ok(mut canvas) => {
                    canvas.set_width(width);
                    canvas.set_height(height);
                }
                Err(_) => ()
            }
            Err(_) => ()
        }
    }

    #[inline]
    pub fn set_min_dimensions(&self, _dimensions: Option<LogicalSize>) {
        // N/A
    }

    #[inline]
    pub fn set_max_dimensions(&self, _dimensions: Option<LogicalSize>) {
        // N/A
    }

    #[inline]
    pub fn set_resizable(&self, _resizable: bool) {
        // N/A
    }

    #[inline]
    pub fn show(&self) {
        // N/A
    }

    #[inline]
    pub fn hide(&self) {
        // N/A
    }

    #[inline]
    pub fn set_cursor(&self, _cursor: ::MouseCursor) {
        // N/A
    }

    #[inline]
    pub fn grab_cursor(&self, grab: bool) -> Result<(), String> {
        let mut grabbed_lock = self.window.cursor_grabbed.lock().unwrap();
        if grab == *grabbed_lock { return Ok(()); }
        //TODO: if grab if applicable, set the appropriate callback
        *grabbed_lock = grab;
        Ok(())
    }

    #[inline]
    pub fn hide_cursor(&self, hide: bool) {
        let mut hidden_lock = self.window.cursor_hidden.lock().unwrap();
        if hide == *hidden_lock { return; }
        if hide {
            // TODO: hide mouse with stdweb
        } else {
            show_mouse();
        }
        *hidden_lock = hide;
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f64 {
        get_hidpi_factor()
    }

    #[inline]
    pub fn set_cursor_position(&self, _position: LogicalPosition) -> Result<(), String> {
        Err("Setting cursor position is not possible on Emscripten.".to_owned())
    }

    #[inline]
    pub fn set_maximized(&self, _maximized: bool) {
        // iOS has single screen maximized apps so nothing to do
    }

    #[inline]
    pub fn set_fullscreen(&self, _monitor: Option<::MonitorId>) {
        // iOS has single screen maximized apps so nothing to do
    }

    #[inline]
    pub fn set_decorations(&self, _decorations: bool) {
        // N/A
    }

    #[inline]
    pub fn set_always_on_top(&self, _always_on_top: bool) {
        // N/A
    }

    #[inline]
    pub fn set_window_icon(&self, _icon: Option<::Icon>) {
        // N/A
    }

    #[inline]
    pub fn set_ime_spot(&self, _logical_spot: LogicalPosition) {
        // N/A
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
}

impl Drop for Window {
    fn drop(&mut self) {
        // Delete window from events_loop
        // TODO: ?
        /*if let Some(ev) = self.events_loop.upgrade() {
            let _ = ev.window.lock().unwrap().take().unwrap();
        }*/
        // Return back to normal cursor state
        self.hide_cursor(false);
        self.grab_cursor(false);

        
        if self.window.is_fullscreen {
            // TODO: Exit fullscreen if on
        }

        //TODO: delete callbacks
    }
}

fn key_translate(input: String) -> u8 {
    if key.chars().count() == 1 {
        key.as_bytes()[0]
    } else {
        0
    }
}

fn key_translate_virt(input: &str,
                      location: KeyboardLocation) -> Option<::VirtualKeyCode>
{
    use VirtualKeyCode::*;
    match input {
        "Alt" => match location {
            KeyboardLocation::Left => Some(LAlt),
            KeyboardLocation::Right => Some(RAlt),
            _ => None,
        },
        "AltGraph" => None,
        "CapsLock" => None,
        "Control" => match location {
            KeyboardLocation::Left => Some(LControl),
            KeyboardLocation::Right => Some(RControl),
            _ => None,
        },
        "Fn" => None,
        "FnLock" => None,
        "Hyper" => None,
        "Meta" => None,
        "NumLock" => Some(Numlock),
        "ScrollLock" => Some(Scroll),
        "Shift" => match location {
            KeyboardLocation::Left => Some(LShift),
            KeyboardLocation::Right => Some(RShift),
            _ => None,
        },
        "Super" => None,
        "Symbol" => None,
        "SymbolLock" => None,

        "Enter" => match location {
            KeyboardLocation::Numpad => Some(NumpadEnter),
            _ => Some(Return),
        },
        "Tab" => Some(Tab),
        " " => Some(Space),

        "ArrowDown" => Some(Down),
        "ArrowLeft" => Some(Left),
        "ArrowRight" => Some(Right),
        "ArrowUp" => Some(Up),
        "End" => None,
        "Home" => None,
        "PageDown" => None,
        "PageUp" => None,

        "Backspace" => Some(Back),
        "Clear" => None,
        "Copy" => None,
        "CrSel" => None,
        "Cut" => None,
        "Delete" => None,
        "EraseEof" => None,
        "ExSel" => None,
        "Insert" => Some(Insert),
        "Paste" => None,
        "Redo" => None,
        "Undo" => None,

        "Accept" => None,
        "Again" => None,
        "Attn" => None,
        "Cancel" => None,
        "ContextMenu" => None,
        "Escape" => Some(Escape),
        "Execute" => None,
        "Find" => None,
        "Finish" => None,
        "Help" => None,
        "Pause" => Some(Pause),
        "Play" => None,
        "Props" => None,
        "Select" => None,
        "ZoomIn" => None,
        "ZoomOut" => None,

        "BrightnessDown" => None,
        "BrightnessUp" => None,
        "Eject" => None,
        "LogOff" => None,
        "Power" => Some(Power),
        "PowerOff" => None,
        "PrintScreen" => Some(Snapshot),
        "Hibernate" => None,
        "Standby" => Some(Sleep),
        "WakeUp" => Some(Wake),

        "AllCandidates" => None,
        "Alphanumeric" => None,
        "CodeInput" => None,
        "Compose" => Some(Compose),
        "Convert" => Some(Convert),
        "Dead" => None,
        "FinalMode" => None,
        "GroupFirst" => None,
        "GroupLast" => None,
        "GroupNext" => None,
        "GroupPrevious" => None,
        "ModeChange" => None,
        "NextCandidate" => None,
        "NonConvert" => None,
        "PreviousCandidate" => None,
        "Process" => None,
        "SingleCandidate" => None,

        "HangulMode" => None,
        "HanjaMode" => None,
        "JunjaMode" => None,

        "Eisu" => None,
        "Hankaku" => None,
        "Hiragana" => None,
        "HiraganaKatakana" => None,
        "KanaMode" => Some(Kana),
        "KanjiMode" => Some(Kanji),
        "Romaji" => None,
        "Zenkaku" => None,
        "ZenkakuHanaku" => None,

        "F1" => Some(F1),
        "F2" => Some(F2),
        "F3" => Some(F3),
        "F4" => Some(F4),
        "F5" => Some(F5),
        "F6" => Some(F6),
        "F7" => Some(F7),
        "F8" => Some(F8),
        "F9" => Some(F9),
        "F10" => Some(F10),
        "F11" => Some(F11),
        "F12" => Some(F12),
        "F13" => Some(F13),
        "F14" => Some(F14),
        "F15" => Some(F15),
        "F16" => None,
        "F17" => None,
        "F18" => None,
        "F19" => None,
        "F20" => None,
        "Soft1" => None,
        "Soft2" => None,
        "Soft3" => None,
        "Soft4" => None,

        "AppSwitch" => None,
        "Call" => None,
        "Camera" => None,
        "CameraFocus" => None,
        "EndCall" => None,
        "GoBack" => None,
        "GoHome" => None,
        "HeadsetHook" => None,
        "LastNumberRedial" => None,
        "Notification" => None,
        "MannerMode" => None,
        "VoiceDial" => None,

        "ChannelDown" => None,
        "ChannelUp" => None,
        "MediaFastForward" => None,
        "MediaPause" => None,
        "MediaPlay" => None,
        "MediaPlayPause" => Some(PlayPause),
        "MediaRecord" => None,
        "MediaRewind" => None,
        "MediaStop" => Some(MediaStop),
        "MediaTrackNext" => Some(NextTrack),
        "MediaTrackPrevious" => Some(PrevTrack),

        "AudioBalanceLeft" => None,
        "AudioBalanceRight" => None,
        "AudioBassDown" => None,
        "AudioBassBoostDown" => None,
        "AudioBassBoostToggle" => None,
        "AudioBassBoostUp" => None,
        "AudioBassUp" => None,
        "AudioFaderFront" => None,
        "AudioFaderRear" => None,
        "AudioSurroundModeNext" => None,
        "AudioTrebleDown" => None,
        "AudioTrebleUp" => None,
        "AudioVolumeDown" => Some(VolumeDown),
        "AudioVolumeMute" => Some(Mute),
        "AudioVolumeUp" => Some(VolumeUp),
        "MicrophoneToggle" => None,
        "MicrophoneVolumeDown" => None,
        "MicrophoneVolumeMute" => None,
        "MicrophoneVolumeUp" => None,

        "TV" => None,
        "TV3DMode" => None,
        "TVAntennaCable" => None,
        "TVAudioDescription" => None,
        "TVAudioDescriptionMixDown" => None,
        "TVAudioDescriptionMixUp" => None,
        "TVContentsMenu" => None,
        "TVDataService" => None,
        "TVInput" => None,
        "TVInputComponent1" => None,
        "TVInputComponent2" => None,
        "TVInputComposite1" => None,
        "TVInputComposite2" => None,
        "TVInputHDM1" => None,
        "TVInputHDM2" => None,
        "TVInputHDM3" => None,
        "TVInputHDM4" => None,
        "TVInputVGA1" => None,
        "TVMediaContext" => None,
        "TVNetwork" => None,
        "TVNumberEntry" => None,
        "TVPower" => None,
        "TVRadioService" => None,
        "TVSatellite" => None,
        "TVSatelliteBS" => None,
        "TVSatelliteCS" => None,
        "TVSatelliteToggle" => None,
        "TVTerrestrialAnalog" => None,
        "TVTerrestrialDigital" => None,
        "TVTimer" => None,

        "AVRInput" => None,
        "AVRPower" => None,
        "ColorF0Red" => None,
        "ColorF1Green" => None,
        "ColorF2Yellow" => None,
        "ColorF3Blue" => None,
        "ColorF4Grey" => None,
        "ColorF5Brown" => None,
        "ClosedCaptionToggle" => None,
        "Dimmer" => None,
        "DisplaySwap" => None,
        "DVR" => None,
        "Exit" => None,
        "FavoriteClear0" => None,
        "FavoriteClear1" => None,
        "FavoriteClear2" => None,
        "FavoriteClear3" => None,
        "FavoriteRecall0" => None,
        "FavoriteRecall1" => None,
        "FavoriteRecall2" => None,
        "FavoriteRecall3" => None,
        "FavoriteStore0" => None,
        "FavoriteStore1" => None,
        "FavoriteStore2" => None,
        "FavoriteStore3" => None,
        "FavoriteStore4" => None,
        "Guide" => None,
        "GuideNextDay" => None,
        "GuidePreviousDay" => None,
        "Info" => None,
        "InstantReplay" => None,
        "Link" => None,
        "ListProgram" => None,
        "LiveContent" => None,
        "Lock" => None,
        "MediaApps" => None,
        "MediaAudioTrack" => None,
        "MediaLast" => None,
        "MediaSkipBackward" => None,
        "MediaSkipForward" => None,
        "MediaStepBackward" => None,
        "MediaStepForward" => None,
        "MediaTopMenu" => None,
        "NavigateIn" => None,
        "NavigateNext" => None,
        "NavigateOut" => None,
        "NavigatePrevious" => None,
        "NextFavoriteChannel" => None,
        "NextUserProfile" => None,
        "OnDemand" => None,
        "Pairing" => None,
        "PinPDown" => None,
        "PinPMove" => None,
        "PinPToggle" => None,
        "PinPUp" => None,
        "PlaySpeedDown" => None,
        "PlaySpeedReset" => None,
        "PlaySpeedUp" => None,
        "RandomToggle" => None,
        "RcLowBattery" => None,
        "RecordSpeedNext" => None,
        "RfBypass" => None,
        "ScanChannelsToggle" => None,
        "ScreenModeNext" => None,
        "Settings" => None,
        "SplitScreenToggle" => None,
        "STBInput" => None,
        "STBPower" => None,
        "Subtitle" => None,
        "Teletext" => None,
        "VideoModeNext" => None,
        "Wink" => None,
        "ZoomToggle" => None,

        "SpeechCorrectionList" => None,
        "SpeechInputToggle" => None,

        "Close" => None,
        "New" => None,
        "Open" => None,
        "Print" => None,
        "Save" => None,
        "SpellCheck" => None,
        "MailForward" => None,
        "MailReply" => None,
        "MailSend" => None,

        "LaunchCalculator" => Some(Calculator),
        "LaunchCalendar" => None,
        "LaunchContacts" => None,
        "LaunchMail" => Some(Mail),
        "LaunchMediaPlayer" => None,
        "LaunchMusicPlayer" => None,
        "LaunchMyComputer" => Some(MyComputer),
        "LaunchPhone" => None,
        "LaunchScreenSaver" => None,
        "LaunchSpreadsheet" => None,
        "LaunchWebCam" => None,
        "LaunchWordProcessor" => None,
        "LaunchApplication1" => None,
        "LaunchApplication2" => None,
        "LaunchApplication3" => None,
        "LaunchApplication4" => None,
        "LaunchApplication5" => None,
        "LaunchApplication6" => None,
        "LaunchApplication7" => None,
        "LaunchApplication8" => None,
        "LaunchApplication9" => None,
        "LaunchApplication10" => None,
        "LaunchApplication11" => None,
        "LaunchApplication12" => None,
        "LaunchApplication13" => None,
        "LaunchApplication14" => None,
        "LaunchApplication15" => None,
        "LaunchApplication16" => None,

        "BrowserBack" => Some(WebBack),
        "BrowserFavorites" => Some(WebFavorites),
        "BrowserForward" => Some(WebForward),
        "BrowserHome" => Some(WebHome),
        "BrowserRefresh" => Some(WebRefresh),
        "BrowserSearch" => Some(WebSearch),
        "BrowserStop" => Some(WebStop),

        "Decimal" => Some(Decimal),
        "Key11" => None,
        "Key12" => None,
        "Multiply" | "*" => Some(Multiply),
        "Add" | "+" => Some(Add),
        // "Clear" => None,
        "Divide" => Some(Divide),
        "Subtract" | "-" => Some(Subtract),
        "Separator" => None,
        "0" => match location {
            KeyboardLocation::Numpad => Some(Numpad0),
            _ => Some(Key0),
        },
        "1" => match location {
            KeyboardLocation::Numpad => Some(Numpad1),
            _ => Some(Key1),
        },
        "2" => match location {
            KeyboardLocation::Numpad => Some(Numpad2),
            _ => Some(Key2),
        },
        "3" => match location {
            KeyboardLocation::Numpad => Some(Numpad3),
            _ => Some(Key3),
        },
        "4" => match location {
            KeyboardLocation::Numpad => Some(Numpad4),
            _ => Some(Key4),
        },
        "5" => match location {
            KeyboardLocation::Numpad => Some(Numpad5),
            _ => Some(Key5),
        },
        "6" => match location {
            KeyboardLocation::Numpad => Some(Numpad6),
            _ => Some(Key6),
        },
        "7" => match location {
            KeyboardLocation::Numpad => Some(Numpad7),
            _ => Some(Key7),
        },
        "8" => match location {
            KeyboardLocation::Numpad => Some(Numpad8),
            _ => Some(Key8),
        },
        "9" => match location {
            KeyboardLocation::Numpad => Some(Numpad9),
            _ => Some(Key9),
        },

        "A" | "a" => Some(A),
        "B" | "b" => Some(B),
        "C" | "c" => Some(C),
        "D" | "d" => Some(D),
        "E" | "e" => Some(E),
        "F" | "f" => Some(F),
        "G" | "g" => Some(G),
        "H" | "h" => Some(H),
        "I" | "i" => Some(I),
        "J" | "j" => Some(J),
        "K" | "k" => Some(K),
        "L" | "l" => Some(L),
        "M" | "m" => Some(M),
        "N" | "n" => Some(N),
        "O" | "o" => Some(O),
        "P" | "p" => Some(P),
        "Q" | "q" => Some(Q),
        "R" | "r" => Some(R),
        "S" | "s" => Some(S),
        "T" | "t" => Some(T),
        "U" | "u" => Some(U),
        "V" | "v" => Some(V),
        "W" | "w" => Some(W),
        "X" | "x" => Some(X),
        "Y" | "y" => Some(Y),
        "Z" | "z" => Some(Z),

        "'" => Some(Apostrophe),
        "\\" => Some(Backslash),
        ":" => Some(Colon),
        "," => match location {
            KeyboardLocation::Numpad => Some(NumpadComma),
            _ => Some(Comma),
        },
        "=" => match location {
            KeyboardLocation::Numpad => Some(NumpadEquals),
            _ => Some(Equals),
        },
        "{" => Some(LBracket),
        "." => Some(Period),
        "}" => Some(RBracket),
        ";" => Some(Semicolon),
        "/" => Some(Slash),

        _ => None,
    }
}