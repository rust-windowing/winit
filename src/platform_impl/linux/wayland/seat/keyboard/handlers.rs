//! Handling of various keyboard events.

use sctk::reexports::calloop;
use sctk::reexports::client;
use sctk::reexports::client::protocol::wl_keyboard::KeyState;

use crate::event::{ElementState, KeyEvent, WindowEvent};
use crate::keyboard::{Key, ModifiersState, NativeKeyCode};
use crate::platform_impl::platform::common::keymap;
use crate::platform_impl::platform::common::xkb_state::{self, RMLVO};
use crate::platform_impl::wayland::event_loop::WinitState;
use crate::platform_impl::wayland::{self, DeviceId};
use crate::platform_impl::KeyEventExtra;

use super::KeyboardInner;

#[inline]
pub(super) fn handle_keyboard(
    event: Event<'_>,
    inner: &mut KeyboardInner,
    winit_state: &mut WinitState,
) {
    let event_sink = &mut winit_state.event_sink;
    match event {
        Event::Enter { surface, .. } => {
            let window_id = wayland::make_wid(&surface);

            // Window gained focus.
            event_sink.push_window_event(WindowEvent::Focused(true), window_id);

            // Dispatch modifers changes that we've received before getting `Enter` event.
            if let Some(modifiers) = inner.pending_modifers_state.take() {
                *inner.modifiers_state.borrow_mut() = modifiers;
                event_sink.push_window_event(WindowEvent::ModifiersChanged(modifiers), window_id);
            }

            inner.target_window_id = Some(window_id);
        }
        Event::Leave { surface, .. } => {
            let window_id = wayland::make_wid(&surface);

            // Notify that no modifiers are being pressed.
            if !inner.modifiers_state.borrow().is_empty() {
                event_sink.push_window_event(
                    WindowEvent::ModifiersChanged(ModifiersState::empty()),
                    window_id,
                );
            }

            // Window lost focus.
            event_sink.push_window_event(WindowEvent::Focused(false), window_id);

            // Reset the id.
            inner.target_window_id = None;
        }
        Event::Key {
            rawkey,
            keysym,
            state,
            utf8,
            ..
        } => {
            let window_id = match inner.target_window_id {
                Some(window_id) => window_id,
                None => return,
            };

            let state = match state {
                KeyState::Pressed => ElementState::Pressed,
                KeyState::Released => ElementState::Released,
                _ => unreachable!(),
            };

            let physical_key = keymap::rawkey_to_keycode(rawkey);
            let logical_key = keymap::keysym_to_key(keysym);
            let location = keymap::keysym_location(keysym);

            event_sink.push_window_event(
                WindowEvent::KeyboardInput {
                    device_id: crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(
                        DeviceId,
                    )),
                    event: KeyEvent {
                        physical_key,
                        logical_key,
                        text: None,
                        location,
                        state,
                        repeat: false,
                        platform_specific: KeyEventExtra {
                            key_without_modifiers: Key::Unidentified(NativeKeyCode::Unidentified),
                            text_with_all_modifers: None,
                        },
                    },
                    is_synthetic: false,
                },
                window_id,
            );

            // Send ReceivedCharacter event only on ElementState::Pressed.
            if ElementState::Released == state {
                return;
            }

            if let Some(txt) = utf8 {
                for ch in txt.chars() {
                    // event_sink.push_window_event(WindowEvent::ReceivedCharacter(ch), window_id);
                }
            }
        }
        Event::Repeat {
            rawkey,
            keysym,
            utf8,
            ..
        } => {
            let window_id = match inner.target_window_id {
                Some(window_id) => window_id,
                None => return,
            };

            let physical_key = keymap::rawkey_to_keycode(rawkey);
            let logical_key = keymap::keysym_to_key(keysym);
            let location = keymap::keysym_location(keysym);

            event_sink.push_window_event(
                WindowEvent::KeyboardInput {
                    device_id: crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(
                        DeviceId,
                    )),
                    event: KeyEvent {
                        physical_key,
                        logical_key,
                        text: None,
                        location,
                        state: ElementState::Pressed,
                        repeat: true,
                        platform_specific: KeyEventExtra {
                            key_without_modifiers: Key::Unidentified(NativeKeyCode::Unidentified),
                            text_with_all_modifers: None,
                        },
                    },
                    is_synthetic: false,
                },
                window_id,
            );

            if let Some(txt) = utf8 {
                for ch in txt.chars() {
                    // event_sink.push_window_event(WindowEvent::ReceivedCharacter(ch), window_id);
                }
            }
        }
        Event::Modifiers { modifiers } => {
            let modifiers = ModifiersState::from(modifiers);
            if let Some(window_id) = inner.target_window_id {
                *inner.modifiers_state.borrow_mut() = modifiers;

                event_sink.push_window_event(WindowEvent::ModifiersChanged(modifiers), window_id);
            } else {
                // Compositor must send modifiers after wl_keyboard::enter, however certain
                // compositors are still sending it before, so stash such events and send
                // them on wl_keyboard::enter.
                inner.pending_modifers_state = Some(modifiers);
            }
        }
    }
}

// ! ====================================================================================================== !
// !                                         "INSPIRED" BY SCTK                                             !
// ! ====================================================================================================== !

use std::num::NonZeroU32;
use std::time::Duration;
use std::{
    cell::RefCell,
    convert::TryInto,
    fs::File,
    os::unix::io::{FromRawFd, RawFd},
    rc::Rc,
};

use sctk::reexports::client::{
    protocol::{wl_keyboard, wl_seat, wl_surface},
    Attached,
};
use xkbcommon_dl as ffi;

use super::super::super::super::common::xkb_state::KbState;

const MICROS_IN_SECOND: u32 = 1000000;

/// Possible kinds of key repetition
pub enum RepeatKind {
    /// keys will be repeated at a set rate and delay
    Fixed {
        /// The number of repetitions per second that should occur.
        rate: u32,
        /// delay (in milliseconds) between a key press and the start of repetition
        delay: u32,
    },
    /// keys will be repeated at a rate and delay set by the wayland server
    System,
}

#[derive(Debug)]
/// An error that occurred while trying to initialize a mapped keyboard
pub enum Error {
    XkbState(xkb_state::Error),
    /// The provided seat does not have the keyboard capability
    NoKeyboard,
    /// Failed to init timers for repetition
    TimerError(std::io::Error),
}

impl From<xkb_state::Error> for Error {
    fn from(err: xkb_state::Error) -> Self {
        Self::XkbState(err)
    }
}

/// Events received from a mapped keyboard
pub enum Event<'a> {
    /// The keyboard focus has entered a surface
    Enter {
        /// serial number of the event
        serial: u32,
        /// surface that was entered
        surface: wl_surface::WlSurface,
        /// raw values of the currently pressed keys
        rawkeys: &'a [u32],
        /// interpreted symbols of the currently pressed keys
        keysyms: &'a [u32],
    },
    /// The keyboard focus has left a surface
    Leave {
        /// serial number of the event
        serial: u32,
        /// surface that was left
        surface: wl_surface::WlSurface,
    },
    /// The key modifiers have changed state
    Modifiers {
        /// current state of the modifiers
        modifiers: xkb_state::ModifiersState,
    },
    /// A key event occurred
    Key {
        /// serial number of the event
        serial: u32,
        /// time at which the keypress occurred
        time: u32,
        /// raw value of the key
        rawkey: u32,
        /// interpreted symbol of the key
        keysym: u32,
        /// new state of the key
        state: KeyState,
        /// utf8 interpretation of the entered text
        ///
        /// will always be `None` on key release events
        utf8: Option<String>,
    },
    /// A key repetition event
    Repeat {
        /// time at which the repetition occured
        time: u32,
        /// raw value of the key
        rawkey: u32,
        /// interpreted symbol of the key
        keysym: u32,
        /// utf8 interpretation of the entered text
        utf8: Option<String>,
    },
}

/// Implement a keyboard for keymap translation with key repetition
///
/// This requires you to provide a callback to receive the events after they
/// have been interpreted with the keymap.
///
/// The keymap will be loaded from the provided RMLVO rules, or from the compositor
/// provided keymap if `None`.
///
/// Returns an error if xkbcommon could not be initialized, the RMLVO specification
/// contained invalid values, or if the provided seat does not have keyboard capability.
pub fn map_keyboard_repeat<F, Data: 'static>(
    loop_handle: calloop::LoopHandle<'static, Data>,
    seat: &Attached<wl_seat::WlSeat>,
    rmlvo: Option<RMLVO>,
    repeatkind: RepeatKind,
    callback: F,
) -> Result<(wl_keyboard::WlKeyboard, calloop::RegistrationToken), Error>
where
    F: FnMut(Event<'_>, wl_keyboard::WlKeyboard, wayland_client::DispatchData<'_>) + 'static,
{
    let has_kbd = sctk::seat::with_seat_data(seat, |data| data.has_keyboard).unwrap_or(false);
    let keyboard = if has_kbd {
        seat.get_keyboard()
    } else {
        return Err(Error::NoKeyboard);
    };

    let state = Rc::new(RefCell::new(
        rmlvo
            .map(KbState::from_rmlvo)
            .unwrap_or_else(KbState::new)?,
    ));

    let callback = Rc::new(RefCell::new(callback));

    let repeat = match repeatkind {
        RepeatKind::System => RepeatDetails {
            locked: false,
            gap: None,
            delay: 200,
        },
        RepeatKind::Fixed { rate, delay } => {
            let gap = rate_to_gap(rate as i32);
            RepeatDetails {
                locked: true,
                gap,
                delay,
            }
        }
    };

    // Prepare the repetition handling.
    let (mut kbd_handler, source) = {
        let current_repeat = Rc::new(RefCell::new(None));

        let source = RepeatSource {
            timer: calloop::timer::Timer::new().map_err(Error::TimerError)?,
            state: state.clone(),
            current_repeat: current_repeat.clone(),
        };

        let timer_handle = source.timer.handle();

        let handler = KbdHandler {
            callback: callback.clone(),
            state,
            repeat: Some(KbdRepeat {
                timer_handle,
                current_repeat,
                details: repeat,
            }),
        };
        (handler, source)
    };

    let source = loop_handle
        .insert_source(source, move |event, kbd, ddata| {
            (&mut *callback.borrow_mut())(
                event,
                kbd.clone(),
                wayland_client::DispatchData::wrap(ddata),
            )
        })
        .map_err(|e| Error::TimerError(e.error))?;

    keyboard.quick_assign(move |keyboard, event, data| {
        kbd_handler.event(keyboard.detach(), event, data)
    });

    Ok((keyboard.detach(), source))
}

fn rate_to_gap(rate: i32) -> Option<NonZeroU32> {
    if rate <= 0 {
        None
    } else if MICROS_IN_SECOND < rate as u32 {
        NonZeroU32::new(1)
    } else {
        NonZeroU32::new(MICROS_IN_SECOND / rate as u32)
    }
}

/*
 * Classic handling
 */

type KbdCallback = dyn FnMut(Event<'_>, wl_keyboard::WlKeyboard, wayland_client::DispatchData<'_>);

struct RepeatDetails {
    locked: bool,
    /// Gap between key presses in microseconds.
    ///
    /// If the `gap` is `None`, it means that repeat is disabled.
    gap: Option<NonZeroU32>,
    /// Delay before starting key repeat in milliseconds.
    delay: u32,
}

struct KbdHandler {
    state: Rc<RefCell<KbState>>,
    callback: Rc<RefCell<KbdCallback>>,
    repeat: Option<KbdRepeat>,
}

struct KbdRepeat {
    timer_handle: calloop::timer::TimerHandle<()>,
    current_repeat: Rc<RefCell<Option<RepeatData>>>,
    details: RepeatDetails,
}

impl KbdRepeat {
    fn start_repeat(&self, key: u32, keyboard: wl_keyboard::WlKeyboard, time: u32) {
        // Start a new repetition, overwriting the previous ones
        self.timer_handle.cancel_all_timeouts();

        // Handle disabled repeat rate.
        let gap = match self.details.gap {
            Some(gap) => gap.get() as u64,
            None => return,
        };

        *self.current_repeat.borrow_mut() = Some(RepeatData {
            keyboard,
            keycode: key,
            gap,
            time: (time + self.details.delay) as u64 * 1000,
        });
        self.timer_handle
            .add_timeout(Duration::from_micros(self.details.delay as u64 * 1000), ());
    }

    fn stop_repeat(&self, key: u32) {
        // only cancel if the released key is the currently repeating key
        let mut guard = self.current_repeat.borrow_mut();
        let stop = (*guard).as_ref().map(|d| d.keycode == key).unwrap_or(false);
        if stop {
            self.timer_handle.cancel_all_timeouts();
            *guard = None;
        }
    }

    fn stop_all_repeat(&self) {
        self.timer_handle.cancel_all_timeouts();
        *self.current_repeat.borrow_mut() = None;
    }
}

impl KbdHandler {
    fn event(
        &mut self,
        kbd: wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        dispatch_data: client::DispatchData<'_>,
    ) {
        use wl_keyboard::Event;

        match event {
            Event::Keymap { format, fd, size } => self.keymap(kbd, format, fd, size),
            Event::Enter {
                serial,
                surface,
                keys,
            } => self.enter(kbd, serial, surface, keys, dispatch_data),
            Event::Leave { serial, surface } => self.leave(kbd, serial, surface, dispatch_data),
            Event::Key {
                serial,
                time,
                key,
                state,
            } => self.key(kbd, serial, time, key, state, dispatch_data),
            Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => self.modifiers(
                kbd,
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                dispatch_data,
            ),
            Event::RepeatInfo { rate, delay } => self.repeat_info(kbd, rate, delay),
            _ => {}
        }
    }

    fn keymap(
        &mut self,
        _: wl_keyboard::WlKeyboard,
        format: wl_keyboard::KeymapFormat,
        fd: RawFd,
        size: u32,
    ) {
        let fd = unsafe { File::from_raw_fd(fd) };
        let mut state = self.state.borrow_mut();
        if state.locked() {
            // state is locked, ignore keymap updates
            return;
        }
        if state.ready() {
            // new keymap, we first deinit to free resources
            unsafe {
                state.de_init();
            }
        }
        match format {
            wl_keyboard::KeymapFormat::XkbV1 => unsafe {
                state.init_with_fd(fd, size as usize);
            },
            wl_keyboard::KeymapFormat::NoKeymap => {
                // TODO: how to handle this (hopefully never occuring) case?
            }
            _ => unreachable!(),
        }
    }

    fn enter(
        &mut self,
        object: wl_keyboard::WlKeyboard,
        serial: u32,
        surface: wl_surface::WlSurface,
        keys: Vec<u8>,
        dispatch_data: client::DispatchData<'_>,
    ) {
        let mut state = self.state.borrow_mut();
        let rawkeys = keys
            .chunks_exact(4)
            .map(|c| u32::from_ne_bytes(c.try_into().unwrap()))
            .collect::<Vec<_>>();
        let keys: Vec<u32> = rawkeys.iter().map(|k| state.get_one_sym_raw(*k)).collect();
        (&mut *self.callback.borrow_mut())(
            Event::Enter {
                serial,
                surface,
                rawkeys: &rawkeys,
                keysyms: &keys,
            },
            object,
            dispatch_data,
        );
    }

    fn leave(
        &mut self,
        object: wl_keyboard::WlKeyboard,
        serial: u32,
        surface: wl_surface::WlSurface,
        dispatch_data: client::DispatchData<'_>,
    ) {
        {
            if let Some(ref mut repeat) = self.repeat {
                repeat.stop_all_repeat();
            }
        }
        (&mut *self.callback.borrow_mut())(Event::Leave { serial, surface }, object, dispatch_data);
    }

    fn key(
        &mut self,
        object: wl_keyboard::WlKeyboard,
        serial: u32,
        time: u32,
        key: u32,
        key_state: wl_keyboard::KeyState,
        dispatch_data: client::DispatchData<'_>,
    ) {
        let (sym, utf8, repeats) = {
            let mut state = self.state.borrow_mut();
            // Get the values to generate a key event
            let sym = state.get_one_sym_raw(key);
            let utf8 = if key_state == wl_keyboard::KeyState::Pressed {
                match state.compose_feed(sym) {
                    Some(ffi::xkb_compose_feed_result::XKB_COMPOSE_FEED_ACCEPTED) => {
                        if let Some(status) = state.compose_status() {
                            match status {
                                ffi::xkb_compose_status::XKB_COMPOSE_COMPOSED => {
                                    state.compose_get_utf8()
                                }
                                ffi::xkb_compose_status::XKB_COMPOSE_NOTHING => {
                                    state.get_utf8_raw(key)
                                }
                                _ => None,
                            }
                        } else {
                            state.get_utf8_raw(key)
                        }
                    }
                    Some(_) => {
                        // XKB_COMPOSE_FEED_IGNORED
                        None
                    }
                    None => {
                        // XKB COMPOSE is not initialized
                        state.get_utf8_raw(key)
                    }
                }
            } else {
                None
            };
            let repeats = unsafe { state.key_repeats(key + 8) };
            (sym, utf8, repeats)
        };

        {
            if let Some(ref mut repeat_handle) = self.repeat {
                if repeats {
                    if key_state == wl_keyboard::KeyState::Pressed {
                        repeat_handle.start_repeat(key, object.clone(), time);
                    } else {
                        repeat_handle.stop_repeat(key);
                    }
                }
            }
        }

        (&mut *self.callback.borrow_mut())(
            Event::Key {
                serial,
                time,
                rawkey: key,
                keysym: sym,
                state: key_state,
                utf8,
            },
            object,
            dispatch_data,
        );
    }

    fn modifiers(
        &mut self,
        object: wl_keyboard::WlKeyboard,
        mods_depressed: u32,
        mods_latched: u32,
        mods_locked: u32,
        group: u32,
        dispatch_data: client::DispatchData<'_>,
    ) {
        {
            let mut state = self.state.borrow_mut();
            state.update_modifiers(mods_depressed, mods_latched, mods_locked, group);
            (&mut *self.callback.borrow_mut())(
                Event::Modifiers {
                    modifiers: state.mods_state(),
                },
                object,
                dispatch_data,
            );
        }
    }

    fn repeat_info(&mut self, _: wl_keyboard::WlKeyboard, rate: i32, delay: i32) {
        {
            if let Some(ref mut repeat_handle) = self.repeat {
                if !repeat_handle.details.locked {
                    repeat_handle.details.gap = rate_to_gap(rate);
                    repeat_handle.details.delay = delay as u32;
                }
            }
        }
    }
}

/*
 * Repeat handling
 */

struct RepeatData {
    keyboard: wl_keyboard::WlKeyboard,
    keycode: u32,
    /// Gap between key presses in microseconds.
    gap: u64,
    /// Time of the last event in microseconds.
    time: u64,
}

/// An event source managing the key repetition of a keyboard
///
/// It is given to you from [`map_keyboard`](fn.map_keyboard.html), and you need to
/// insert it in your calloop event loop if you want to have functionning key repetition.
///
/// If don't want key repetition you can just drop it.
///
/// This source will not directly generate calloop events, and the callback provided to
/// `EventLoopHandle::insert_source()` will be ignored. Instead it triggers the
/// callback you provided to [`map_keyboard`](fn.map_keyboard.html).
pub struct RepeatSource {
    timer: calloop::timer::Timer<()>,
    state: Rc<RefCell<KbState>>,
    current_repeat: Rc<RefCell<Option<RepeatData>>>,
}

impl calloop::EventSource for RepeatSource {
    type Event = Event<'static>;
    type Metadata = wl_keyboard::WlKeyboard;
    type Ret = ();

    fn process_events<F>(
        &mut self,
        readiness: calloop::Readiness,
        token: calloop::Token,
        mut callback: F,
    ) -> std::io::Result<calloop::PostAction>
    where
        F: FnMut(Event<'static>, &mut wl_keyboard::WlKeyboard),
    {
        let current_repeat = &self.current_repeat;
        let state = &self.state;
        self.timer
            .process_events(readiness, token, |(), timer_handle| {
                if let Some(ref mut data) = *current_repeat.borrow_mut() {
                    // there is something to repeat
                    let mut state = state.borrow_mut();
                    let keysym = state.get_one_sym_raw(data.keycode);
                    let utf8 = state.get_utf8_raw(data.keycode);
                    let new_time = data.gap + data.time;
                    // Notify the callback.
                    callback(
                        Event::Repeat {
                            time: (new_time / 1000) as u32,
                            rawkey: data.keycode,
                            keysym,
                            utf8,
                        },
                        &mut data.keyboard,
                    );
                    // Update the time of last event.
                    data.time = new_time;
                    // Schedule the next timeout.
                    timer_handle.add_timeout(Duration::from_micros(data.gap), ());
                }
            })
    }

    fn register(
        &mut self,
        poll: &mut calloop::Poll,
        token_factory: &mut calloop::TokenFactory,
    ) -> std::io::Result<()> {
        self.timer.register(poll, token_factory)
    }

    fn reregister(
        &mut self,
        poll: &mut calloop::Poll,
        token_factory: &mut calloop::TokenFactory,
    ) -> std::io::Result<()> {
        self.timer.reregister(poll, token_factory)
    }

    fn unregister(&mut self, poll: &mut calloop::Poll) -> std::io::Result<()> {
        self.timer.unregister(poll)
    }
}
