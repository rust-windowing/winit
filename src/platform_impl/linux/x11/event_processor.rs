use std::{cell::RefCell, collections::HashMap, rc::Rc, slice, sync::Arc};

use libc::{c_char, c_int, c_long, c_uint, c_ulong};

use super::{
    events, ffi, get_xtarget, mkdid, mkwid, monitor, util, Device, DeviceId, DeviceInfo, Dnd,
    DndState, GenericEventCookie, ImeReceiver, ScrollOrientation, UnownedWindow, WindowId,
    XExtension,
};

use util::modifiers::{ModifierKeyState, ModifierKeymap};

use crate::platform_impl::platform::x11::ime::{ImeEvent, ImeEventReceiver, ImeRequest};
use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{
        DeviceEvent, ElementState, Event, Ime, KeyboardInput, ModifiersState, TouchPhase,
        WindowEvent,
    },
    event_loop::EventLoopWindowTarget as RootELW,
};

/// The X11 documentation states: "Keycodes lie in the inclusive range `[8, 255]`".
const KEYCODE_OFFSET: u8 = 8;

pub(super) struct EventProcessor<T: 'static> {
    pub(super) dnd: Dnd,
    pub(super) ime_receiver: ImeReceiver,
    pub(super) ime_event_receiver: ImeEventReceiver,
    pub(super) randr_event_offset: c_int,
    pub(super) devices: RefCell<HashMap<DeviceId, Device>>,
    pub(super) xi2ext: XExtension,
    pub(super) target: Rc<RootELW<T>>,
    pub(super) mod_keymap: ModifierKeymap,
    pub(super) device_mod_state: ModifierKeyState,
    // Number of touch events currently in progress
    pub(super) num_touch: u32,
    pub(super) first_touch: Option<u64>,
    // Currently focused window belonging to this process
    pub(super) active_window: Option<ffi::Window>,
    pub(super) is_composing: bool,
}

impl<T: 'static> EventProcessor<T> {
    pub(super) fn init_device(&self, device: c_int) {
        let wt = get_xtarget(&self.target);
        let mut devices = self.devices.borrow_mut();
        if let Some(info) = DeviceInfo::get(&wt.xconn, device) {
            for info in info.iter() {
                devices.insert(DeviceId(info.deviceid), Device::new(info));
            }
        }
    }

    fn with_window<F, Ret>(&self, window_id: ffi::Window, callback: F) -> Option<Ret>
    where
        F: Fn(&Arc<UnownedWindow>) -> Ret,
    {
        let mut deleted = false;
        let window_id = WindowId(window_id as _);
        let wt = get_xtarget(&self.target);
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

    fn window_exists(&self, window_id: ffi::Window) -> bool {
        self.with_window(window_id, |_| ()).is_some()
    }

    pub(super) fn poll(&self) -> bool {
        let wt = get_xtarget(&self.target);
        let result = unsafe { (wt.xconn.xlib.XPending)(wt.xconn.display) };

        result != 0
    }

    pub(super) unsafe fn poll_one_event(&mut self, event_ptr: *mut ffi::XEvent) -> bool {
        let wt = get_xtarget(&self.target);
        // This function is used to poll and remove a single event
        // from the Xlib event queue in a non-blocking, atomic way.
        // XCheckIfEvent is non-blocking and removes events from queue.
        // XNextEvent can't be used because it blocks while holding the
        // global Xlib mutex.
        // XPeekEvent does not remove events from the queue.
        unsafe extern "C" fn predicate(
            _display: *mut ffi::Display,
            _event: *mut ffi::XEvent,
            _arg: *mut c_char,
        ) -> c_int {
            // This predicate always returns "true" (1) to accept all events
            1
        }

        let result = (wt.xconn.xlib.XCheckIfEvent)(
            wt.xconn.display,
            event_ptr,
            Some(predicate),
            std::ptr::null_mut(),
        );

        result != 0
    }

    pub(super) fn process_event<F>(&mut self, xev: &mut ffi::XEvent, mut callback: F)
    where
        F: FnMut(Event<'_, T>),
    {
        let wt = get_xtarget(&self.target);
        // XFilterEvent tells us when an event has been discarded by the input method.
        // Specifically, this involves all of the KeyPress events in compose/pre-edit sequences,
        // along with an extra copy of the KeyRelease events. This also prevents backspace and
        // arrow keys from being detected twice.
        if ffi::True
            == unsafe {
                (wt.xconn.xlib.XFilterEvent)(xev, {
                    let xev: &ffi::XAnyEvent = xev.as_ref();
                    xev.window
                })
            }
        {
            return;
        }

        // We can't call a `&mut self` method because of the above borrow,
        // so we use this macro for repeated modifier state updates.
        macro_rules! update_modifiers {
            ( $state:expr , $modifier:expr ) => {{
                match ($state, $modifier) {
                    (state, modifier) => {
                        if let Some(modifiers) =
                            self.device_mod_state.update_state(&state, modifier)
                        {
                            if let Some(window_id) = self.active_window {
                                callback(Event::WindowEvent {
                                    window_id: mkwid(window_id),
                                    event: WindowEvent::ModifiersChanged(modifiers),
                                });
                            }
                        }
                    }
                }
            }};
        }

        let event_type = xev.get_type();
        match event_type {
            ffi::MappingNotify => {
                let mapping: &ffi::XMappingEvent = xev.as_ref();

                if mapping.request == ffi::MappingModifier
                    || mapping.request == ffi::MappingKeyboard
                {
                    unsafe {
                        (wt.xconn.xlib.XRefreshKeyboardMapping)(xev.as_mut());
                    }
                    wt.xconn
                        .check_errors()
                        .expect("Failed to call XRefreshKeyboardMapping");

                    self.mod_keymap.reset_from_x_connection(&wt.xconn);
                    self.device_mod_state.update_keymap(&self.mod_keymap);
                }
            }

            ffi::ClientMessage => {
                let client_msg: &ffi::XClientMessageEvent = xev.as_ref();

                let window = client_msg.window;
                let window_id = mkwid(window);

                if client_msg.data.get_long(0) as ffi::Atom == wt.wm_delete_window {
                    callback(Event::WindowEvent {
                        window_id,
                        event: WindowEvent::CloseRequested,
                    });
                } else if client_msg.data.get_long(0) as ffi::Atom == wt.net_wm_ping {
                    let response_msg: &mut ffi::XClientMessageEvent = xev.as_mut();
                    response_msg.window = wt.root;
                    wt.xconn
                        .send_event(
                            wt.root,
                            Some(ffi::SubstructureNotifyMask | ffi::SubstructureRedirectMask),
                            *response_msg,
                        )
                        .queue();
                } else if client_msg.message_type == self.dnd.atoms.enter {
                    let source_window = client_msg.data.get_long(0) as c_ulong;
                    let flags = client_msg.data.get_long(1);
                    let version = flags >> 24;
                    self.dnd.version = Some(version);
                    let has_more_types = flags - (flags & (c_long::max_value() - 1)) == 1;
                    if !has_more_types {
                        let type_list = vec![
                            client_msg.data.get_long(2) as c_ulong,
                            client_msg.data.get_long(3) as c_ulong,
                            client_msg.data.get_long(4) as c_ulong,
                        ];
                        self.dnd.type_list = Some(type_list);
                    } else if let Ok(more_types) = unsafe { self.dnd.get_type_list(source_window) }
                    {
                        self.dnd.type_list = Some(more_types);
                    }
                } else if client_msg.message_type == self.dnd.atoms.position {
                    // This event occurs every time the mouse moves while a file's being dragged
                    // over our window. We emit HoveredFile in response; while the macOS backend
                    // does that upon a drag entering, XDND doesn't have access to the actual drop
                    // data until this event. For parity with other platforms, we only emit
                    // `HoveredFile` the first time, though if winit's API is later extended to
                    // supply position updates with `HoveredFile` or another event, implementing
                    // that here would be trivial.

                    let source_window = client_msg.data.get_long(0) as c_ulong;

                    // Equivalent to `(x << shift) | y`
                    // where `shift = mem::size_of::<c_short>() * 8`
                    // Note that coordinates are in "desktop space", not "window space"
                    // (in X11 parlance, they're root window coordinates)
                    //let packed_coordinates = client_msg.data.get_long(2);
                    //let shift = mem::size_of::<libc::c_short>() * 8;
                    //let x = packed_coordinates >> shift;
                    //let y = packed_coordinates & !(x << shift);

                    // By our own state flow, `version` should never be `None` at this point.
                    let version = self.dnd.version.unwrap_or(5);

                    // Action is specified in versions 2 and up, though we don't need it anyway.
                    //let action = client_msg.data.get_long(4);

                    let accepted = if let Some(ref type_list) = self.dnd.type_list {
                        type_list.contains(&self.dnd.atoms.uri_list)
                    } else {
                        false
                    };

                    if accepted {
                        self.dnd.source_window = Some(source_window);
                        unsafe {
                            if self.dnd.result.is_none() {
                                let time = if version >= 1 {
                                    client_msg.data.get_long(3) as c_ulong
                                } else {
                                    // In version 0, time isn't specified
                                    ffi::CurrentTime
                                };
                                // This results in the `SelectionNotify` event below
                                self.dnd.convert_selection(window, time);
                            }
                            self.dnd
                                .send_status(window, source_window, DndState::Accepted)
                                .expect("Failed to send `XdndStatus` message.");
                        }
                    } else {
                        unsafe {
                            self.dnd
                                .send_status(window, source_window, DndState::Rejected)
                                .expect("Failed to send `XdndStatus` message.");
                        }
                        self.dnd.reset();
                    }
                } else if client_msg.message_type == self.dnd.atoms.drop {
                    let (source_window, state) = if let Some(source_window) = self.dnd.source_window
                    {
                        if let Some(Ok(ref path_list)) = self.dnd.result {
                            for path in path_list {
                                callback(Event::WindowEvent {
                                    window_id,
                                    event: WindowEvent::DroppedFile(path.clone()),
                                });
                            }
                        }
                        (source_window, DndState::Accepted)
                    } else {
                        // `source_window` won't be part of our DND state if we already rejected the drop in our
                        // `XdndPosition` handler.
                        let source_window = client_msg.data.get_long(0) as c_ulong;
                        (source_window, DndState::Rejected)
                    };
                    unsafe {
                        self.dnd
                            .send_finished(window, source_window, state)
                            .expect("Failed to send `XdndFinished` message.");
                    }
                    self.dnd.reset();
                } else if client_msg.message_type == self.dnd.atoms.leave {
                    self.dnd.reset();
                    callback(Event::WindowEvent {
                        window_id,
                        event: WindowEvent::HoveredFileCancelled,
                    });
                }
            }

            ffi::SelectionNotify => {
                let xsel: &ffi::XSelectionEvent = xev.as_ref();

                let window = xsel.requestor;
                let window_id = mkwid(window);

                if xsel.property == self.dnd.atoms.selection {
                    let mut result = None;

                    // This is where we receive data from drag and drop
                    if let Ok(mut data) = unsafe { self.dnd.read_data(window) } {
                        let parse_result = self.dnd.parse_data(&mut data);
                        if let Ok(ref path_list) = parse_result {
                            for path in path_list {
                                callback(Event::WindowEvent {
                                    window_id,
                                    event: WindowEvent::HoveredFile(path.clone()),
                                });
                            }
                        }
                        result = Some(parse_result);
                    }

                    self.dnd.result = result;
                }
            }

            ffi::ConfigureNotify => {
                let xev: &ffi::XConfigureEvent = xev.as_ref();
                let xwindow = xev.window;
                let window_id = mkwid(xwindow);

                if let Some(window) = self.with_window(xwindow, Arc::clone) {
                    // So apparently...
                    // `XSendEvent` (synthetic `ConfigureNotify`) -> position relative to root
                    // `XConfigureNotify` (real `ConfigureNotify`) -> position relative to parent
                    // https://tronche.com/gui/x/icccm/sec-4.html#s-4.1.5
                    // We don't want to send `Moved` when this is false, since then every `Resized`
                    // (whether the window moved or not) is accompanied by an extraneous `Moved` event
                    // that has a position relative to the parent window.
                    let is_synthetic = xev.send_event == ffi::True;

                    // These are both in physical space.
                    let new_inner_size = (xev.width as u32, xev.height as u32);
                    let new_inner_position = (xev.x, xev.y);

                    let (mut resized, moved) = {
                        let mut shared_state_lock = window.shared_state_lock();

                        let resized =
                            util::maybe_change(&mut shared_state_lock.size, new_inner_size);
                        let moved = if is_synthetic {
                            util::maybe_change(
                                &mut shared_state_lock.inner_position,
                                new_inner_position,
                            )
                        } else {
                            // Detect when frame extents change.
                            // Since this isn't synthetic, as per the notes above, this position is relative to the
                            // parent window.
                            let rel_parent = new_inner_position;
                            if util::maybe_change(
                                &mut shared_state_lock.inner_position_rel_parent,
                                rel_parent,
                            ) {
                                // This ensures we process the next `Moved`.
                                shared_state_lock.inner_position = None;
                                // Extra insurance against stale frame extents.
                                shared_state_lock.frame_extents = None;
                            }
                            false
                        };
                        (resized, moved)
                    };

                    let position = window.shared_state_lock().position;

                    let new_outer_position = if let (Some(position), false) = (position, moved) {
                        position
                    } else {
                        let mut shared_state_lock = window.shared_state_lock();

                        // We need to convert client area position to window position.
                        let frame_extents = shared_state_lock
                            .frame_extents
                            .as_ref()
                            .cloned()
                            .unwrap_or_else(|| {
                                let frame_extents =
                                    wt.xconn.get_frame_extents_heuristic(xwindow, wt.root);
                                shared_state_lock.frame_extents = Some(frame_extents.clone());
                                frame_extents
                            });
                        let outer = frame_extents
                            .inner_pos_to_outer(new_inner_position.0, new_inner_position.1);
                        shared_state_lock.position = Some(outer);

                        // Unlock shared state to prevent deadlock in callback below
                        drop(shared_state_lock);

                        if moved {
                            callback(Event::WindowEvent {
                                window_id,
                                event: WindowEvent::Moved(outer.into()),
                            });
                        }
                        outer
                    };

                    if is_synthetic {
                        let mut shared_state_lock = window.shared_state_lock();
                        // If we don't use the existing adjusted value when available, then the user can screw up the
                        // resizing by dragging across monitors *without* dropping the window.
                        let (width, height) = shared_state_lock
                            .dpi_adjusted
                            .unwrap_or((xev.width as u32, xev.height as u32));

                        let last_scale_factor = shared_state_lock.last_monitor.scale_factor;
                        let new_scale_factor = {
                            let window_rect = util::AaRect::new(new_outer_position, new_inner_size);
                            let monitor = wt.xconn.get_monitor_for_window(Some(window_rect));

                            if monitor.is_dummy() {
                                // Avoid updating monitor using a dummy monitor handle
                                last_scale_factor
                            } else {
                                shared_state_lock.last_monitor = monitor.clone();
                                monitor.scale_factor
                            }
                        };
                        if last_scale_factor != new_scale_factor {
                            let (new_width, new_height) = window.adjust_for_dpi(
                                last_scale_factor,
                                new_scale_factor,
                                width,
                                height,
                                &shared_state_lock,
                            );

                            let old_inner_size = PhysicalSize::new(width, height);
                            let mut new_inner_size = PhysicalSize::new(new_width, new_height);

                            // Unlock shared state to prevent deadlock in callback below
                            drop(shared_state_lock);

                            callback(Event::WindowEvent {
                                window_id,
                                event: WindowEvent::ScaleFactorChanged {
                                    scale_factor: new_scale_factor,
                                    new_inner_size: &mut new_inner_size,
                                },
                            });

                            if new_inner_size != old_inner_size {
                                window.set_inner_size_physical(
                                    new_inner_size.width,
                                    new_inner_size.height,
                                );
                                window.shared_state_lock().dpi_adjusted =
                                    Some(new_inner_size.into());
                                // if the DPI factor changed, force a resize event to ensure the logical
                                // size is computed with the right DPI factor
                                resized = true;
                            }
                        }
                    }

                    let mut shared_state_lock = window.shared_state_lock();

                    // This is a hack to ensure that the DPI adjusted resize is actually applied on all WMs. KWin
                    // doesn't need this, but Xfwm does. The hack should not be run on other WMs, since tiling
                    // WMs constrain the window size, making the resize fail. This would cause an endless stream of
                    // XResizeWindow requests, making Xorg, the winit client, and the WM consume 100% of CPU.
                    if let Some(adjusted_size) = shared_state_lock.dpi_adjusted {
                        if new_inner_size == adjusted_size || !util::wm_name_is_one_of(&["Xfwm4"]) {
                            // When this finally happens, the event will not be synthetic.
                            shared_state_lock.dpi_adjusted = None;
                        } else {
                            window.set_inner_size_physical(adjusted_size.0, adjusted_size.1);
                        }
                    }

                    // Unlock shared state to prevent deadlock in callback below
                    drop(shared_state_lock);

                    if resized {
                        callback(Event::WindowEvent {
                            window_id,
                            event: WindowEvent::Resized(new_inner_size.into()),
                        });
                    }
                }
            }

            ffi::ReparentNotify => {
                let xev: &ffi::XReparentEvent = xev.as_ref();

                // This is generally a reliable way to detect when the window manager's been
                // replaced, though this event is only fired by reparenting window managers
                // (which is almost all of them). Failing to correctly update WM info doesn't
                // really have much impact, since on the WMs affected (xmonad, dwm, etc.) the only
                // effect is that we waste some time trying to query unsupported properties.
                wt.xconn.update_cached_wm_info(wt.root);

                self.with_window(xev.window, |window| {
                    window.invalidate_cached_frame_extents();
                });
            }
            ffi::MapNotify => {
                let xev: &ffi::XMapEvent = xev.as_ref();
                let window = xev.window;
                let window_id = mkwid(window);

                // XXX re-issue the focus state when mapping the window.
                //
                // The purpose of it is to deliver initial focused state of the newly created
                // window, given that we can't rely on `CreateNotify`, due to it being not
                // sent.
                let focus = self
                    .with_window(window, |window| window.has_focus())
                    .unwrap_or_default();
                callback(Event::WindowEvent {
                    window_id,
                    event: WindowEvent::Focused(focus),
                });
            }
            ffi::DestroyNotify => {
                let xev: &ffi::XDestroyWindowEvent = xev.as_ref();

                let window = xev.window;
                let window_id = mkwid(window);

                // In the event that the window's been destroyed without being dropped first, we
                // cleanup again here.
                wt.windows.borrow_mut().remove(&WindowId(window as _));

                // Since all XIM stuff needs to happen from the same thread, we destroy the input
                // context here instead of when dropping the window.
                wt.ime
                    .borrow_mut()
                    .remove_context(window)
                    .expect("Failed to destroy input context");

                callback(Event::WindowEvent {
                    window_id,
                    event: WindowEvent::Destroyed,
                });
            }

            ffi::VisibilityNotify => {
                let xev: &ffi::XVisibilityEvent = xev.as_ref();
                let xwindow = xev.window;
                callback(Event::WindowEvent {
                    window_id: mkwid(xwindow),
                    event: WindowEvent::Occluded(xev.state == ffi::VisibilityFullyObscured),
                });
                self.with_window(xwindow, |window| {
                    window.visibility_notify();
                });
            }

            ffi::Expose => {
                let xev: &ffi::XExposeEvent = xev.as_ref();

                // Multiple Expose events may be received for subareas of a window.
                // We issue `RedrawRequested` only for the last event of such a series.
                if xev.count == 0 {
                    let window = xev.window;
                    let window_id = mkwid(window);

                    callback(Event::RedrawRequested(window_id));
                }
            }

            ffi::KeyPress | ffi::KeyRelease => {
                use crate::event::ElementState::{Pressed, Released};

                // Note that in compose/pre-edit sequences, this will always be Released.
                let state = if xev.get_type() == ffi::KeyPress {
                    Pressed
                } else {
                    Released
                };

                let xkev: &mut ffi::XKeyEvent = xev.as_mut();

                let window = xkev.window;
                let window_id = mkwid(window);

                // Standard virtual core keyboard ID. XInput2 needs to be used to get a reliable
                // value, though this should only be an issue under multiseat configurations.
                let device = util::VIRTUAL_CORE_KEYBOARD;
                let device_id = mkdid(device);
                let keycode = xkev.keycode;

                // When a compose sequence or IME pre-edit is finished, it ends in a KeyPress with
                // a keycode of 0.
                if keycode != 0 && !self.is_composing {
                    let scancode = keycode - KEYCODE_OFFSET as u32;
                    let keysym = wt.xconn.lookup_keysym(xkev);
                    let virtual_keycode = events::keysym_to_element(keysym as c_uint);

                    update_modifiers!(
                        ModifiersState::from_x11_mask(xkev.state),
                        self.mod_keymap.get_modifier(xkev.keycode as ffi::KeyCode)
                    );

                    let modifiers = self.device_mod_state.modifiers();

                    #[allow(deprecated)]
                    callback(Event::WindowEvent {
                        window_id,
                        event: WindowEvent::KeyboardInput {
                            device_id,
                            input: KeyboardInput {
                                state,
                                scancode,
                                virtual_keycode,
                                modifiers,
                            },
                            is_synthetic: false,
                        },
                    });
                }

                if state == Pressed {
                    let written = if let Some(ic) = wt.ime.borrow().get_context(window) {
                        wt.xconn.lookup_utf8(ic, xkev)
                    } else {
                        return;
                    };

                    // If we're composing right now, send the string we've got from X11 via
                    // Ime::Commit.
                    if self.is_composing && keycode == 0 && !written.is_empty() {
                        let event = Event::WindowEvent {
                            window_id,
                            event: WindowEvent::Ime(Ime::Preedit(String::new(), None)),
                        };
                        callback(event);

                        let event = Event::WindowEvent {
                            window_id,
                            event: WindowEvent::Ime(Ime::Commit(written)),
                        };

                        self.is_composing = false;
                        callback(event);
                    } else {
                        for chr in written.chars() {
                            let event = Event::WindowEvent {
                                window_id,
                                event: WindowEvent::ReceivedCharacter(chr),
                            };

                            callback(event);
                        }
                    }
                }
            }

            ffi::GenericEvent => {
                let guard = if let Some(e) = GenericEventCookie::from_event(&wt.xconn, *xev) {
                    e
                } else {
                    return;
                };
                let xev = &guard.cookie;
                if self.xi2ext.opcode != xev.extension {
                    return;
                }

                use crate::event::{
                    ElementState::{Pressed, Released},
                    MouseButton::{Left, Middle, Other, Right},
                    MouseScrollDelta::LineDelta,
                    Touch,
                    WindowEvent::{
                        AxisMotion, CursorEntered, CursorLeft, CursorMoved, Focused, MouseInput,
                        MouseWheel,
                    },
                };

                match xev.evtype {
                    ffi::XI_ButtonPress | ffi::XI_ButtonRelease => {
                        let xev: &ffi::XIDeviceEvent = unsafe { &*(xev.data as *const _) };
                        let window_id = mkwid(xev.event);
                        let device_id = mkdid(xev.deviceid);
                        if (xev.flags & ffi::XIPointerEmulated) != 0 {
                            // Deliver multi-touch events instead of emulated mouse events.
                            return;
                        }

                        let modifiers = ModifiersState::from_x11(&xev.mods);
                        update_modifiers!(modifiers, None);

                        let state = if xev.evtype == ffi::XI_ButtonPress {
                            Pressed
                        } else {
                            Released
                        };
                        match xev.detail as u32 {
                            ffi::Button1 => callback(Event::WindowEvent {
                                window_id,
                                event: MouseInput {
                                    device_id,
                                    state,
                                    button: Left,
                                    modifiers,
                                },
                            }),
                            ffi::Button2 => callback(Event::WindowEvent {
                                window_id,
                                event: MouseInput {
                                    device_id,
                                    state,
                                    button: Middle,
                                    modifiers,
                                },
                            }),
                            ffi::Button3 => callback(Event::WindowEvent {
                                window_id,
                                event: MouseInput {
                                    device_id,
                                    state,
                                    button: Right,
                                    modifiers,
                                },
                            }),

                            // Suppress emulated scroll wheel clicks, since we handle the real motion events for those.
                            // In practice, even clicky scroll wheels appear to be reported by evdev (and XInput2 in
                            // turn) as axis motion, so we don't otherwise special-case these button presses.
                            4 | 5 | 6 | 7 => {
                                if xev.flags & ffi::XIPointerEmulated == 0 {
                                    callback(Event::WindowEvent {
                                        window_id,
                                        event: MouseWheel {
                                            device_id,
                                            delta: match xev.detail {
                                                4 => LineDelta(0.0, 1.0),
                                                5 => LineDelta(0.0, -1.0),
                                                6 => LineDelta(1.0, 0.0),
                                                7 => LineDelta(-1.0, 0.0),
                                                _ => unreachable!(),
                                            },
                                            phase: TouchPhase::Moved,
                                            modifiers,
                                        },
                                    });
                                }
                            }

                            x => callback(Event::WindowEvent {
                                window_id,
                                event: MouseInput {
                                    device_id,
                                    state,
                                    button: Other(x as u16),
                                    modifiers,
                                },
                            }),
                        }
                    }
                    ffi::XI_Motion => {
                        let xev: &ffi::XIDeviceEvent = unsafe { &*(xev.data as *const _) };
                        let device_id = mkdid(xev.deviceid);
                        let window_id = mkwid(xev.event);
                        let new_cursor_pos = (xev.event_x, xev.event_y);

                        let modifiers = ModifiersState::from_x11(&xev.mods);
                        update_modifiers!(modifiers, None);

                        let cursor_moved = self.with_window(xev.event, |window| {
                            let mut shared_state_lock = window.shared_state_lock();
                            util::maybe_change(&mut shared_state_lock.cursor_pos, new_cursor_pos)
                        });
                        if cursor_moved == Some(true) {
                            let position = PhysicalPosition::new(xev.event_x, xev.event_y);

                            callback(Event::WindowEvent {
                                window_id,
                                event: CursorMoved {
                                    device_id,
                                    position,
                                    modifiers,
                                },
                            });
                        } else if cursor_moved.is_none() {
                            return;
                        }

                        // More gymnastics, for self.devices
                        let mut events = Vec::new();
                        {
                            let mask = unsafe {
                                slice::from_raw_parts(
                                    xev.valuators.mask,
                                    xev.valuators.mask_len as usize,
                                )
                            };
                            let mut devices = self.devices.borrow_mut();
                            let physical_device = match devices.get_mut(&DeviceId(xev.sourceid)) {
                                Some(device) => device,
                                None => return,
                            };

                            let mut value = xev.valuators.values;
                            for i in 0..xev.valuators.mask_len * 8 {
                                if ffi::XIMaskIsSet(mask, i) {
                                    let x = unsafe { *value };
                                    if let Some(&mut (_, ref mut info)) = physical_device
                                        .scroll_axes
                                        .iter_mut()
                                        .find(|&&mut (axis, _)| axis == i)
                                    {
                                        let delta = (x - info.position) / info.increment;
                                        info.position = x;
                                        events.push(Event::WindowEvent {
                                            window_id,
                                            event: MouseWheel {
                                                device_id,
                                                delta: match info.orientation {
                                                    // X11 vertical scroll coordinates are opposite to winit's
                                                    ScrollOrientation::Horizontal => {
                                                        LineDelta(-delta as f32, 0.0)
                                                    }
                                                    ScrollOrientation::Vertical => {
                                                        LineDelta(0.0, -delta as f32)
                                                    }
                                                },
                                                phase: TouchPhase::Moved,
                                                modifiers,
                                            },
                                        });
                                    } else {
                                        events.push(Event::WindowEvent {
                                            window_id,
                                            event: AxisMotion {
                                                device_id,
                                                axis: i as u32,
                                                value: unsafe { *value },
                                            },
                                        });
                                    }
                                    value = unsafe { value.offset(1) };
                                }
                            }
                        }
                        for event in events {
                            callback(event);
                        }
                    }

                    ffi::XI_Enter => {
                        let xev: &ffi::XIEnterEvent = unsafe { &*(xev.data as *const _) };

                        let window_id = mkwid(xev.event);
                        let device_id = mkdid(xev.deviceid);

                        if let Some(all_info) = DeviceInfo::get(&wt.xconn, ffi::XIAllDevices) {
                            let mut devices = self.devices.borrow_mut();
                            for device_info in all_info.iter() {
                                if device_info.deviceid == xev.sourceid
                                // This is needed for resetting to work correctly on i3, and
                                // presumably some other WMs. On those, `XI_Enter` doesn't include
                                // the physical device ID, so both `sourceid` and `deviceid` are
                                // the virtual device.
                                || device_info.attachment == xev.sourceid
                                {
                                    let device_id = DeviceId(device_info.deviceid);
                                    if let Some(device) = devices.get_mut(&device_id) {
                                        device.reset_scroll_position(device_info);
                                    }
                                }
                            }
                        }

                        if self.window_exists(xev.event) {
                            callback(Event::WindowEvent {
                                window_id,
                                event: CursorEntered { device_id },
                            });

                            let position = PhysicalPosition::new(xev.event_x, xev.event_y);

                            // The mods field on this event isn't actually populated, so query the
                            // pointer device. In the future, we can likely remove this round-trip by
                            // relying on `Xkb` for modifier values.
                            //
                            // This needs to only be done after confirming the window still exists,
                            // since otherwise we risk getting a `BadWindow` error if the window was
                            // dropped with queued events.
                            let modifiers = wt
                                .xconn
                                .query_pointer(xev.event, xev.deviceid)
                                .expect("Failed to query pointer device")
                                .get_modifier_state();

                            callback(Event::WindowEvent {
                                window_id,
                                event: CursorMoved {
                                    device_id,
                                    position,
                                    modifiers,
                                },
                            });
                        }
                    }
                    ffi::XI_Leave => {
                        let xev: &ffi::XILeaveEvent = unsafe { &*(xev.data as *const _) };

                        // Leave, FocusIn, and FocusOut can be received by a window that's already
                        // been destroyed, which the user presumably doesn't want to deal with.
                        let window_closed = !self.window_exists(xev.event);
                        if !window_closed {
                            callback(Event::WindowEvent {
                                window_id: mkwid(xev.event),
                                event: CursorLeft {
                                    device_id: mkdid(xev.deviceid),
                                },
                            });
                        }
                    }
                    ffi::XI_FocusIn => {
                        let xev: &ffi::XIFocusInEvent = unsafe { &*(xev.data as *const _) };

                        wt.ime
                            .borrow_mut()
                            .focus(xev.event)
                            .expect("Failed to focus input context");

                        let modifiers = ModifiersState::from_x11(&xev.mods);

                        self.device_mod_state.update_state(&modifiers, None);

                        if self.active_window != Some(xev.event) {
                            self.active_window = Some(xev.event);

                            wt.update_device_event_filter(true);

                            let window_id = mkwid(xev.event);
                            let position = PhysicalPosition::new(xev.event_x, xev.event_y);

                            if let Some(window) = self.with_window(xev.event, Arc::clone) {
                                window.shared_state_lock().has_focus = true;
                            }

                            callback(Event::WindowEvent {
                                window_id,
                                event: Focused(true),
                            });

                            if !modifiers.is_empty() {
                                callback(Event::WindowEvent {
                                    window_id,
                                    event: WindowEvent::ModifiersChanged(modifiers),
                                });
                            }

                            // The deviceid for this event is for a keyboard instead of a pointer,
                            // so we have to do a little extra work.
                            let pointer_id = self
                                .devices
                                .borrow()
                                .get(&DeviceId(xev.deviceid))
                                .map(|device| device.attachment)
                                .unwrap_or(2);

                            callback(Event::WindowEvent {
                                window_id,
                                event: CursorMoved {
                                    device_id: mkdid(pointer_id),
                                    position,
                                    modifiers,
                                },
                            });

                            // Issue key press events for all pressed keys
                            Self::handle_pressed_keys(
                                wt,
                                window_id,
                                ElementState::Pressed,
                                &self.mod_keymap,
                                &mut self.device_mod_state,
                                &mut callback,
                            );
                        }
                    }
                    ffi::XI_FocusOut => {
                        let xev: &ffi::XIFocusOutEvent = unsafe { &*(xev.data as *const _) };
                        if !self.window_exists(xev.event) {
                            return;
                        }

                        wt.ime
                            .borrow_mut()
                            .unfocus(xev.event)
                            .expect("Failed to unfocus input context");

                        if self.active_window.take() == Some(xev.event) {
                            let window_id = mkwid(xev.event);

                            wt.update_device_event_filter(false);

                            // Issue key release events for all pressed keys
                            Self::handle_pressed_keys(
                                wt,
                                window_id,
                                ElementState::Released,
                                &self.mod_keymap,
                                &mut self.device_mod_state,
                                &mut callback,
                            );

                            callback(Event::WindowEvent {
                                window_id,
                                event: WindowEvent::ModifiersChanged(ModifiersState::empty()),
                            });

                            if let Some(window) = self.with_window(xev.event, Arc::clone) {
                                window.shared_state_lock().has_focus = false;
                            }

                            callback(Event::WindowEvent {
                                window_id,
                                event: Focused(false),
                            })
                        }
                    }

                    ffi::XI_TouchBegin | ffi::XI_TouchUpdate | ffi::XI_TouchEnd => {
                        let xev: &ffi::XIDeviceEvent = unsafe { &*(xev.data as *const _) };
                        let window_id = mkwid(xev.event);
                        let phase = match xev.evtype {
                            ffi::XI_TouchBegin => TouchPhase::Started,
                            ffi::XI_TouchUpdate => TouchPhase::Moved,
                            ffi::XI_TouchEnd => TouchPhase::Ended,
                            _ => unreachable!(),
                        };
                        if self.window_exists(xev.event) {
                            let id = xev.detail as u64;
                            let modifiers = self.device_mod_state.modifiers();
                            let location = PhysicalPosition::new(xev.event_x, xev.event_y);

                            // Mouse cursor position changes when touch events are received.
                            // Only the first concurrently active touch ID moves the mouse cursor.
                            if is_first_touch(&mut self.first_touch, &mut self.num_touch, id, phase)
                            {
                                callback(Event::WindowEvent {
                                    window_id,
                                    event: WindowEvent::CursorMoved {
                                        device_id: mkdid(util::VIRTUAL_CORE_POINTER),
                                        position: location.cast(),
                                        modifiers,
                                    },
                                });
                            }

                            callback(Event::WindowEvent {
                                window_id,
                                event: WindowEvent::Touch(Touch {
                                    device_id: mkdid(xev.deviceid),
                                    phase,
                                    location,
                                    force: None, // TODO
                                    id,
                                }),
                            })
                        }
                    }

                    ffi::XI_RawButtonPress | ffi::XI_RawButtonRelease => {
                        let xev: &ffi::XIRawEvent = unsafe { &*(xev.data as *const _) };
                        if xev.flags & ffi::XIPointerEmulated == 0 {
                            callback(Event::DeviceEvent {
                                device_id: mkdid(xev.deviceid),
                                event: DeviceEvent::Button {
                                    button: xev.detail as u32,
                                    state: match xev.evtype {
                                        ffi::XI_RawButtonPress => Pressed,
                                        ffi::XI_RawButtonRelease => Released,
                                        _ => unreachable!(),
                                    },
                                },
                            });
                        }
                    }

                    ffi::XI_RawMotion => {
                        let xev: &ffi::XIRawEvent = unsafe { &*(xev.data as *const _) };
                        let did = mkdid(xev.deviceid);

                        let mask = unsafe {
                            slice::from_raw_parts(
                                xev.valuators.mask,
                                xev.valuators.mask_len as usize,
                            )
                        };
                        let mut value = xev.raw_values;
                        let mut mouse_delta = (0.0, 0.0);
                        let mut scroll_delta = (0.0, 0.0);
                        for i in 0..xev.valuators.mask_len * 8 {
                            if ffi::XIMaskIsSet(mask, i) {
                                let x = unsafe { *value };
                                // We assume that every XInput2 device with analog axes is a pointing device emitting
                                // relative coordinates.
                                match i {
                                    0 => mouse_delta.0 = x,
                                    1 => mouse_delta.1 = x,
                                    2 => scroll_delta.0 = x as f32,
                                    3 => scroll_delta.1 = x as f32,
                                    _ => {}
                                }
                                callback(Event::DeviceEvent {
                                    device_id: did,
                                    event: DeviceEvent::Motion {
                                        axis: i as u32,
                                        value: x,
                                    },
                                });
                                value = unsafe { value.offset(1) };
                            }
                        }
                        if mouse_delta != (0.0, 0.0) {
                            callback(Event::DeviceEvent {
                                device_id: did,
                                event: DeviceEvent::MouseMotion { delta: mouse_delta },
                            });
                        }
                        if scroll_delta != (0.0, 0.0) {
                            callback(Event::DeviceEvent {
                                device_id: did,
                                event: DeviceEvent::MouseWheel {
                                    delta: LineDelta(scroll_delta.0, scroll_delta.1),
                                },
                            });
                        }
                    }

                    ffi::XI_RawKeyPress | ffi::XI_RawKeyRelease => {
                        let xev: &ffi::XIRawEvent = unsafe { &*(xev.data as *const _) };

                        let state = match xev.evtype {
                            ffi::XI_RawKeyPress => Pressed,
                            ffi::XI_RawKeyRelease => Released,
                            _ => unreachable!(),
                        };

                        let device_id = mkdid(xev.sourceid);
                        let keycode = xev.detail;
                        let scancode = keycode - KEYCODE_OFFSET as i32;
                        if scancode < 0 {
                            return;
                        }
                        let keysym = wt.xconn.keycode_to_keysym(keycode as ffi::KeyCode);
                        let virtual_keycode = events::keysym_to_element(keysym as c_uint);
                        let modifiers = self.device_mod_state.modifiers();

                        #[allow(deprecated)]
                        callback(Event::DeviceEvent {
                            device_id,
                            event: DeviceEvent::Key(KeyboardInput {
                                scancode: scancode as u32,
                                virtual_keycode,
                                state,
                                modifiers,
                            }),
                        });

                        if let Some(modifier) =
                            self.mod_keymap.get_modifier(keycode as ffi::KeyCode)
                        {
                            self.device_mod_state.key_event(
                                state,
                                keycode as ffi::KeyCode,
                                modifier,
                            );

                            let new_modifiers = self.device_mod_state.modifiers();

                            if modifiers != new_modifiers {
                                if let Some(window_id) = self.active_window {
                                    callback(Event::WindowEvent {
                                        window_id: mkwid(window_id),
                                        event: WindowEvent::ModifiersChanged(new_modifiers),
                                    });
                                }
                            }
                        }
                    }

                    ffi::XI_HierarchyChanged => {
                        let xev: &ffi::XIHierarchyEvent = unsafe { &*(xev.data as *const _) };
                        for info in
                            unsafe { slice::from_raw_parts(xev.info, xev.num_info as usize) }
                        {
                            if 0 != info.flags & (ffi::XISlaveAdded | ffi::XIMasterAdded) {
                                self.init_device(info.deviceid);
                                callback(Event::DeviceEvent {
                                    device_id: mkdid(info.deviceid),
                                    event: DeviceEvent::Added,
                                });
                            } else if 0 != info.flags & (ffi::XISlaveRemoved | ffi::XIMasterRemoved)
                            {
                                callback(Event::DeviceEvent {
                                    device_id: mkdid(info.deviceid),
                                    event: DeviceEvent::Removed,
                                });
                                let mut devices = self.devices.borrow_mut();
                                devices.remove(&DeviceId(info.deviceid));
                            }
                        }
                    }

                    _ => {}
                }
            }
            _ => {
                if event_type == self.randr_event_offset {
                    // In the future, it would be quite easy to emit monitor hotplug events.
                    let prev_list = monitor::invalidate_cached_monitor_list();
                    if let Some(prev_list) = prev_list {
                        let new_list = wt.xconn.available_monitors();
                        for new_monitor in new_list {
                            // Previous list may be empty, in case of disconnecting and
                            // reconnecting the only one monitor. We still need to emit events in
                            // this case.
                            let maybe_prev_scale_factor = prev_list
                                .iter()
                                .find(|prev_monitor| prev_monitor.name == new_monitor.name)
                                .map(|prev_monitor| prev_monitor.scale_factor);
                            if Some(new_monitor.scale_factor) != maybe_prev_scale_factor {
                                for (window_id, window) in wt.windows.borrow().iter() {
                                    if let Some(window) = window.upgrade() {
                                        // Check if the window is on this monitor
                                        let monitor = window.current_monitor();
                                        if monitor.name == new_monitor.name {
                                            let (width, height) = window.inner_size_physical();
                                            let (new_width, new_height) = window.adjust_for_dpi(
                                                // If we couldn't determine the previous scale
                                                // factor (e.g., because all monitors were closed
                                                // before), just pick whatever the current monitor
                                                // has set as a baseline.
                                                maybe_prev_scale_factor
                                                    .unwrap_or(monitor.scale_factor),
                                                new_monitor.scale_factor,
                                                width,
                                                height,
                                                &window.shared_state_lock(),
                                            );

                                            let window_id = crate::window::WindowId(*window_id);
                                            let old_inner_size = PhysicalSize::new(width, height);
                                            let mut new_inner_size =
                                                PhysicalSize::new(new_width, new_height);

                                            callback(Event::WindowEvent {
                                                window_id,
                                                event: WindowEvent::ScaleFactorChanged {
                                                    scale_factor: new_monitor.scale_factor,
                                                    new_inner_size: &mut new_inner_size,
                                                },
                                            });

                                            if new_inner_size != old_inner_size {
                                                let (new_width, new_height) = new_inner_size.into();
                                                window
                                                    .set_inner_size_physical(new_width, new_height);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

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
            Ok((window, event)) => (window, event),
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

    fn handle_pressed_keys<F>(
        wt: &super::EventLoopWindowTarget<T>,
        window_id: crate::window::WindowId,
        state: ElementState,
        mod_keymap: &ModifierKeymap,
        device_mod_state: &mut ModifierKeyState,
        callback: &mut F,
    ) where
        F: FnMut(Event<'_, T>),
    {
        let device_id = mkdid(util::VIRTUAL_CORE_KEYBOARD);
        let modifiers = device_mod_state.modifiers();

        // Update modifiers state and emit key events based on which keys are currently pressed.
        for keycode in wt
            .xconn
            .query_keymap()
            .into_iter()
            .filter(|k| *k >= KEYCODE_OFFSET)
        {
            let scancode = (keycode - KEYCODE_OFFSET) as u32;
            let keysym = wt.xconn.keycode_to_keysym(keycode);
            let virtual_keycode = events::keysym_to_element(keysym as c_uint);

            if let Some(modifier) = mod_keymap.get_modifier(keycode as ffi::KeyCode) {
                device_mod_state.key_event(
                    ElementState::Pressed,
                    keycode as ffi::KeyCode,
                    modifier,
                );
            }

            #[allow(deprecated)]
            callback(Event::WindowEvent {
                window_id,
                event: WindowEvent::KeyboardInput {
                    device_id,
                    input: KeyboardInput {
                        scancode,
                        state,
                        virtual_keycode,
                        modifiers,
                    },
                    is_synthetic: true,
                },
            });
        }
    }
}

fn is_first_touch(first: &mut Option<u64>, num: &mut u32, id: u64, phase: TouchPhase) -> bool {
    match phase {
        TouchPhase::Started => {
            if *num == 0 {
                *first = Some(id);
            }
            *num += 1;
        }
        TouchPhase::Cancelled | TouchPhase::Ended => {
            if *first == Some(id) {
                *first = None;
            }
            *num = num.saturating_sub(1);
        }
        _ => (),
    }

    *first == Some(id)
}
