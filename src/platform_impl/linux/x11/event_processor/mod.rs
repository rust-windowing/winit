use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    convert::Infallible,
    num::NonZeroU8,
    rc::Rc,
    sync::Arc,
};

use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::xproto;

use super::{
    atoms::*, mkdid, mkwid, util, Device, DeviceId, DeviceInfo, Dnd, ImeReceiver, UnownedWindow,
    WindowId, WindowTarget, X11Error,
};

use crate::platform_impl::platform::x11::ime::{ImeEvent, ImeEventReceiver, ImeRequest};
use crate::{
    event::{ElementState, Event, Ime, WindowEvent},
    platform_impl::platform::common::xkb_state::KbdState,
};

// Define an array of event handlers that can be used to process events.
macro_rules! event_handlers {
    ($($key:expr => $fn:expr),* $(,)?) => {
        pub(super) const EVENT_HANDLERS: &[EventHandler] = &[
            $(
                EventHandler {
                    code: $key,
                    handler: |proc, wt, raw_event, cb| {
                        // Parse the event.
                        let (event, _) = match TryParse::try_parse(raw_event) {
                            Ok(event) => event,
                            Err(_) => return,
                        };

                        // Call the handler.
                        let handler = $fn;
                        handler(proc, wt, event, cb)
                    },
                }
            ),*
        ];
    };
}

// These modules define the event handlers.
mod client_message;
mod input;
mod key_press;
mod notify;
mod randr;
mod xkb;

type X11rbEvent = <super::X11rbConnection as RequestConnection>::Buf;

pub(super) struct EventProcessor {
    pub(super) dnd: Dnd,
    pub(super) ime_receiver: ImeReceiver,
    pub(super) ime_event_receiver: ImeEventReceiver,
    pub(super) devices: RefCell<HashMap<DeviceId, Device>>,
    pub(super) kb_state: KbdState,
    // Number of touch events currently in progress
    pub(super) num_touch: u32,
    // This is the last pressed key that is repeatable (if it hasn't been
    // released).
    //
    // Used to detect key repeats.
    pub(super) held_key_press: Option<u32>,
    pub(super) first_touch: Option<u64>,
    // Currently focused window belonging to this process
    pub(super) active_window: Option<xproto::Window>,
    pub(super) is_composing: bool,

    /// The list of enqueued events.
    pub(super) enqueued_events: RefCell<VecDeque<X11rbEvent>>,

    /// Dispatch table for event handlers.
    pub(super) event_handlers: Rc<EventHandlers>,
}

impl EventProcessor {
    pub(super) fn init_device(&self, wt: &WindowTarget, device: u16) {
        let mut devices = self.devices.borrow_mut();
        if let Some(info) = DeviceInfo::get(&wt.xconn, device) {
            for info in info.info.iter() {
                devices.insert(
                    DeviceId(info.deviceid),
                    Device::new(info).expect("no valid device found"),
                );
            }
        }
    }

    pub(crate) fn with_window<F, Ret>(
        &self,
        wt: &super::WindowTarget,
        window_id: xproto::Window,
        callback: F,
    ) -> Option<Ret>
    where
        F: Fn(&Arc<UnownedWindow>) -> Ret,
    {
        let mut deleted = false;
        let window_id = WindowId(window_id as _);
        let result = wt
            .windows
            .borrow()
            .get(&window_id)
            .and_then(|window| {
                let arc = window.upgrade();
                deleted = arc.is_none();
                arc
            })
            .map(|window| callback(&window));
        if deleted {
            // Garbage collection
            wt.windows.borrow_mut().remove(&window_id);
        }
        result
    }

    fn window_exists(&self, wt: &WindowTarget, window_id: xproto::Window) -> bool {
        self.with_window(wt, window_id, |_| ()).is_some()
    }

    /// See if there is an event in the queue.
    pub(super) fn poll(&self, wt: &WindowTarget) -> bool {
        // Check our queue.
        if !self.enqueued_events.borrow().is_empty() {
            return true;
        }

        // Check the display itself for an event.
        if let Ok(Some(event)) = wt.xconn.xcb_connection().poll_for_raw_event() {
            self.enqueued_events.borrow_mut().push_back(event);
        }

        !self.enqueued_events.borrow().is_empty()
    }

    /// Poll for a single event.
    pub(super) fn pop_single_event(
        &self,
        wt: &WindowTarget,
    ) -> Result<Option<X11rbEvent>, X11Error> {
        if let Some(event) = self.enqueued_events.borrow_mut().pop_front() {
            return Ok(Some(event));
        }

        // Try to poll for an event directly.
        wt.xconn
            .xcb_connection()
            .poll_for_raw_event()
            .map_err(Into::into)
    }

    pub(super) fn process_event<T: 'static, F>(
        &mut self,
        wt: &super::EventLoopWindowTarget<T>,
        xev: &[u8],
        mut callback: F,
    ) where
        F: FnMut(Event<T>),
    {
        // Handle the event using our dispatch table.
        self.event_handlers
            .clone()
            .handle_event(self, wt, xev, &mut |event| {
                // Send the event to the callback.
                callback(event.map_nonuser_event().unwrap_or_else(|_| unreachable!()));
            });

        // Handle IME requests.
        if let Ok(request) = self.ime_receiver.try_recv() {
            let mut ime = wt.ime.borrow_mut();
            match request {
                ImeRequest::Position(window_id, x, y) => {
                    ime.send_xim_spot(window_id, x, y);
                }
                ImeRequest::Allow(window_id, allowed) => {
                    ime.set_ime_allowed(window_id, allowed);
                }
            }
        }

        let (window, event) = match self.ime_event_receiver.try_recv() {
            Ok((window, event)) => (window as xproto::Window, event),
            Err(_) => return,
        };

        match event {
            ImeEvent::Enabled => {
                callback(Event::WindowEvent {
                    window_id: mkwid(window),
                    event: WindowEvent::Ime(Ime::Enabled),
                });
            }
            ImeEvent::Start => {
                self.is_composing = true;
                callback(Event::WindowEvent {
                    window_id: mkwid(window),
                    event: WindowEvent::Ime(Ime::Preedit("".to_owned(), None)),
                });
            }
            ImeEvent::Update(text, position) => {
                if self.is_composing {
                    callback(Event::WindowEvent {
                        window_id: mkwid(window),
                        event: WindowEvent::Ime(Ime::Preedit(text, Some((position, position)))),
                    });
                }
            }
            ImeEvent::End => {
                self.is_composing = false;
                // Issue empty preedit on `Done`.
                callback(Event::WindowEvent {
                    window_id: mkwid(window),
                    event: WindowEvent::Ime(Ime::Preedit(String::new(), None)),
                });
            }
            ImeEvent::Disabled => {
                self.is_composing = false;
                callback(Event::WindowEvent {
                    window_id: mkwid(window),
                    event: WindowEvent::Ime(Ime::Disabled),
                });
            }
        }
    }

    fn handle_pressed_keys<T: 'static, F>(
        wt: &super::WindowTarget,
        window_id: crate::window::WindowId,
        state: ElementState,
        kb_state: &mut KbdState,
        callback: &mut F,
    ) where
        F: FnMut(Event<T>),
    {
        let device_id = mkdid(util::VIRTUAL_CORE_KEYBOARD);

        // Update modifiers state and emit key events based on which keys are currently pressed.
        for keycode in wt
            .xconn
            .query_keymap()
            .into_iter()
            .filter(|k| *k >= input::KEYCODE_OFFSET)
        {
            let keycode = keycode as u32;
            let event = kb_state.process_key_event(keycode, state, false);
            callback(Event::WindowEvent {
                window_id,
                event: WindowEvent::KeyboardInput {
                    device_id,
                    event,
                    is_synthetic: true,
                },
            });
        }
    }
}

/// Number of event handlers we need to store.
///
/// It's `0x7F` total events, but `0` and `1` aren't occupied.
const TOTAL_EVENT_HANDLERS: usize = 0x7F - 0x02;

/// The function pointer for event handlers.
type Handler =
    fn(&mut EventProcessor, &super::WindowTarget, &[u8], &mut dyn FnMut(Event<Infallible>));

/// The key used to identify generic events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct GenericKey {
    /// The extension that this event belongs to.
    extension: u8,

    /// The event code for this event.
    code: u16,
}

/// The list of event handlers for the event loop.
///
/// Each X11 event is uniquely identified by an 8-bit code. Rather than using a massive match
/// statement to handle each event, we instead use a lookup table to find a handler we can
/// call.
#[derive(Debug)]
pub(super) struct EventHandlers {
    /// An associative map of event handlers.
    ///
    /// This will be no more than 0x7F entries long, so we don't need a hash map.
    event_handlers: [Handler; TOTAL_EVENT_HANDLERS],

    /// An associative hash map between generic event ID's and handlers.
    generic_events: HashMap<GenericKey, Handler>,
}

impl EventHandlers {
    /// Create a new event handler map.
    pub(super) fn new(conn: &impl Connection) -> Result<Box<Self>, super::X11Error> {
        let mut this: Box<Self> = Box::new(Self {
            event_handlers: [EventHandler::default().handler; TOTAL_EVENT_HANDLERS],
            generic_events: HashMap::new(),
        });

        // Iterate over event handlers and insert them into the map.
        for event_handler in EVENT_HANDLERS.iter().copied().flatten() {
            let code: usize = match event_handler.code {
                EventCode::Generic {
                    extension,
                    event_type,
                } => {
                    let extension_opcode = conn
                        .extension_information(extension)?
                        .expect("TODO: handle missing extensions")
                        .major_opcode;

                    let key = GenericKey {
                        extension: extension_opcode,
                        code: event_type,
                    };

                    this.generic_events.insert(key, event_handler.handler);
                    continue;
                }
                code => code.code(conn)?.get().into(),
            };

            // Insert the event handler.
            this.event_handlers[code - 2] = event_handler.handler;
        }

        Ok(this)
    }

    /// Handle an event.
    fn handle_event(
        &self,
        processor: &mut EventProcessor,
        wt: &super::WindowTarget,
        event: &[u8],
        callback: &mut dyn FnMut(Event<Infallible>),
    ) {
        // Get the event code.
        let code = event[0] & 0x7F;

        // If this is a generic event, it's special.
        if code == xproto::GE_GENERIC_EVENT {
            let generic_event: xproto::GeGenericEvent =
                match x11rb::x11_utils::TryParse::try_parse(event) {
                    Ok((event, _)) => event,
                    Err(e) => {
                        error!("Failed to parse generic event: {:?}", e);
                        return;
                    }
                };

            // Look up the extension code/event type pair.
            let key = GenericKey {
                extension: generic_event.extension,
                code: generic_event.event_type,
            };
            if let Some(handler) = self.generic_events.get(&key) {
                (handler)(processor, wt, event, callback)
            }

            return;
        }

        // Get the event handler.
        let event_handler = self.event_handlers[code as usize - 2];

        // Call the event handler.
        (event_handler)(processor, wt, event, callback)
    }
}

/// A handler for a specific event.
#[derive(Debug, Clone, Copy)]
struct EventHandler {
    /// The event code that this handler is for.
    code: EventCode,

    /// The handler function to call.
    handler: Handler,
}

impl Default for EventHandler {
    fn default() -> Self {
        Self {
            code: EventCode::Xproto(NonZeroU8::new(1).unwrap()),
            handler: |_, _, _, _| {},
        }
    }
}

/// The event code for a handler.
#[derive(Debug, Clone, Copy)]
enum EventCode {
    /// This is just a normal `xproto` event code defined at all times.
    Xproto(NonZeroU8),

    /// This is an extension event code.
    Extension {
        /// The extension that this event code is for.
        extension: &'static str,

        /// The event code offset for this extension.
        offset: u8,
    },

    /// This is a generic event.
    Generic {
        /// The extension that this event code is for.
        extension: &'static str,

        /// The event code for this event.
        event_type: u16,
    },
}

impl EventCode {
    /// Get the code corresponding to this event.
    fn code(self, connection: &impl Connection) -> Result<NonZeroU8, super::X11Error> {
        match self {
            Self::Xproto(code) => {
                debug_assert!(code.get() < 0x7F);
                Ok(code)
            }

            Self::Extension { extension, offset } => {
                // Get extension info for this extension.
                let info = connection
                    .extension_information(extension)?
                    .expect("TODO: handle missing extension");

                // Get the event code.
                let code = info.first_event + offset;

                // Make sure the code is valid.
                debug_assert!(code <= 0x7F);
                let code = NonZeroU8::new(code).expect("TODO: handle invalid event code");

                Ok(code)
            }

            Self::Generic { .. } => Ok(NonZeroU8::new(xproto::GE_GENERIC_EVENT).unwrap()),
        }
    }
}

/// Every event handler that we need to consider.
const EVENT_HANDLERS: &[&[EventHandler]] = &[
    client_message::EVENT_HANDLERS,
    input::EVENT_HANDLERS,
    key_press::EVENT_HANDLERS,
    notify::EVENT_HANDLERS,
    randr::EVENT_HANDLERS,
    xkb::EVENT_HANDLERS,
];

/// Useful imports for the event handlers.
mod prelude {
    pub(super) use super::super::super::WindowId;
    pub(super) use super::super::{
        atoms::*, ffi, mkdid, mkwid, util, CookieResultExt, DeviceId, WindowTarget, ALL_DEVICES,
    };
    pub(super) use super::{EventCode, EventHandler, EventProcessor};

    pub(super) use crate::dpi::{PhysicalPosition, PhysicalSize};
    pub(super) use crate::event::{DeviceEvent, Event, InnerSizeWriter, WindowEvent};

    pub(super) use std::convert::Infallible;
    pub(super) use std::num::NonZeroU8;

    pub(super) use x11rb::protocol::xinput;
    pub(super) use x11rb::protocol::xproto::{self, ConnectionExt as _};
    pub(super) use x11rb::x11_utils::TryParse;

    /// Code for an `xproto` event.
    pub(super) const fn xp_code(code: u8) -> EventCode {
        match code {
            0 => unreachable!(),
            x => EventCode::Xproto(unsafe {
                // SAFETY: We just checked this.
                NonZeroU8::new_unchecked(x)
            }),
        }
    }

    /// Code for an `xinput` event.
    pub(super) const fn xi_code(code: u16) -> EventCode {
        EventCode::Generic {
            extension: xinput::X11_EXTENSION_NAME,
            event_type: code,
        }
    }

    /// Code for an `xkb` event.
    pub(super) const fn xkb_code(code: u8) -> EventCode {
        EventCode::Extension {
            extension: x11rb::protocol::xkb::X11_EXTENSION_NAME,
            offset: code,
        }
    }
}
