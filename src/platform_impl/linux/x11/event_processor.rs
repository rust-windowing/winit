use std::{cell::RefCell, collections::HashMap, convert::identity, rc::Rc, slice, sync::Arc};

use libc::{c_char, c_int, c_long, c_ulong};

use parking_lot::MutexGuard;

use super::{
    ffi, get_xtarget, mkdid, mkwid, monitor, util, Device, DeviceId, DeviceInfo, Dnd, DndState,
    GenericEventCookie, ImeReceiver, ScrollOrientation, UnownedWindow, WindowId, XExtension,
};

use util::modifiers::{ModifierKeyState, ModifierKeymap};

use crate::platform_impl::platform::x11::ime::{ImeEvent, ImeEventReceiver, ImeRequest};
use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{
        DeviceEvent, ElementState, Event, Ime, KeyEvent, RawKeyEvent, TouchPhase, WindowEvent,
    },
    event_loop::EventLoopWindowTarget as RootELW,
    keyboard::ModifiersState,
    platform_impl::platform::{
        common::{keymap, xkb_state::KbState},
        KeyEventExtra,
    },
};

/// The X11 documentation states: "Keycodes lie in the inclusive range [8,255]".
const KEYCODE_OFFSET: u8 = 8;

pub(super) struct EventProcessor<T: 'static> {
    pub(super) dnd: Dnd,
    pub(super) ime_receiver: ImeReceiver,
    pub(super) ime_event_receiver: ImeEventReceiver,
    pub(super) randr_event_offset: c_int,
    pub(super) devices: RefCell<HashMap<DeviceId, Device>>,
    pub(super) xi2ext: XExtension,
    pub(super) xkbext: XExtension,
    pub(super) target: Rc<RootELW<T>>,
    pub(super) kb_state: KbState,
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
        let window_id = WindowId(window_id);
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
                    let new_inner_position = (xev.x as i32, xev.y as i32);

                    let mut shared_state_lock = window.shared_state.lock();

                    let (mut resized, moved) = {
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

                    let new_outer_position = if moved || shared_state_lock.position.is_none() {
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
                        if moved {
                            // Temporarily unlock shared state to prevent deadlock
                            MutexGuard::unlocked(&mut shared_state_lock, || {
                                callback(Event::WindowEvent {
                                    window_id,
                                    event: WindowEvent::Moved(outer.into()),
                                });
                            });
                        }
                        outer
                    } else {
                        shared_state_lock.position.unwrap()
                    };

                    if is_synthetic {
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

                            // Temporarily unlock shared state to prevent deadlock
                            MutexGuard::unlocked(&mut shared_state_lock, || {
                                callback(Event::WindowEvent {
                                    window_id,
                                    event: WindowEvent::ScaleFactorChanged {
                                        scale_factor: new_scale_factor,
                                        new_inner_size: &mut new_inner_size,
                                    },
                                });
                            });

                            if new_inner_size != old_inner_size {
                                window.set_inner_size_physical(
                                    new_inner_size.width,
                                    new_inner_size.height,
                                );
                                shared_state_lock.dpi_adjusted = Some(new_inner_size.into());
                                // if the DPI factor changed, force a resize event to ensure the logical
                                // size is computed with the right DPI factor
                                resized = true;
                            }
                        }
                    }

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

                    if resized {
                        // Drop the shared state lock to prevent deadlock
                        drop(shared_state_lock);

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

            ffi::DestroyNotify => {
                let xev: &ffi::XDestroyWindowEvent = xev.as_ref();

                let window = xev.window;
                let window_id = mkwid(window);

                // In the event that the window's been destroyed without being dropped first, we
                // cleanup again here.
                wt.windows.borrow_mut().remove(&WindowId(window));

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

                self.with_window(xwindow, |window| window.visibility_notify());
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

            // Note that in compose/pre-edit sequences, we'll always receive KeyRelease events
            ffi::KeyPress => {
                // TODO: Is it possible to exclusively use XInput2 events here?
                let xkev: &mut ffi::XKeyEvent = xev.as_mut();

                let window = xkev.window;
                let window_id = mkwid(window);

                let written = if let Some(ic) = wt.ime.borrow().get_context(window) {
                    wt.xconn.lookup_utf8(ic, xkev)
                } else {
                    return;
                };

                // If we're composing right now, send the string we've got from X11 via
                // Ime::Commit.
                if self.is_composing && xkev.keycode == 0 && !written.is_empty() {
                    let event = Event::WindowEvent {
                        window_id,
                        event: WindowEvent::Ime(Ime::Commit(written)),
                    };

                    self.is_composing = false;
                    callback(event);
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
                            let mut shared_state_lock = window.shared_state.lock();
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
                                &mut self.kb_state,
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
                                &mut self.kb_state,
                                &self.mod_keymap,
                                &mut self.device_mod_state,
                                &mut callback,
                            );

                            callback(Event::WindowEvent {
                                window_id,
                                event: WindowEvent::ModifiersChanged(ModifiersState::empty()),
                            });

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
                            let location =
                                PhysicalPosition::new(xev.event_x as f64, xev.event_y as f64);

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

                    // The regular KeyPress event has a problem where if you press a dead key, a KeyPress
                    // event won't be emitted. XInput 2 does not have this problem.
                    ffi::XI_KeyPress | ffi::XI_KeyRelease if !self.is_composing => {
                        if let Some(active_window) = self.active_window {
                            let state = if xev.evtype == ffi::XI_KeyPress {
                                Pressed
                            } else {
                                Released
                            };

                            let xkev: &ffi::XIDeviceEvent = unsafe { &*(xev.data as *const _) };

                            // We use `self.active_window` here as `xkev.event` has a completely different
                            // value for some reason.
                            let window_id = mkwid(active_window);

                            let device_id = mkdid(xkev.deviceid);
                            let keycode = xkev.detail as u32;

                            let keycode = keycode - KEYCODE_OFFSET as u32;
                            let mut ker = self.kb_state.process_key_event(keycode, state);
                            let physical_key = ker.keycode();
                            let (logical_key, location) = ker.key();
                            let text = ker.text();
                            let (key_without_modifiers, _) = ker.key_without_modifiers();
                            let text_with_all_modifiers = ker.text_with_all_modifiers();
                            let repeat = xkev.flags & ffi::XIKeyRepeat == ffi::XIKeyRepeat;

                            callback(Event::WindowEvent {
                                window_id,
                                event: WindowEvent::KeyboardInput {
                                    device_id,
                                    event: KeyEvent {
                                        physical_key,
                                        logical_key,
                                        text,
                                        location,
                                        state,
                                        repeat,
                                        platform_specific: KeyEventExtra {
                                            key_without_modifiers,
                                            text_with_all_modifiers,
                                        },
                                    },
                                    is_synthetic: false,
                                },
                            });
                        }
                    }

                    ffi::XI_RawKeyPress | ffi::XI_RawKeyRelease => {
                        // This is horrible, but I couldn't manage to respect keyboard layout changes
                        // in any other way. In fact, getting this to work at all proved so frustrating
                        // that I (@maroider) lost motivation to work on the keyboard event rework for
                        // some months. Thankfully, @ArturKovacs offered to help debug the problem
                        // over discord, and the following is the result of that debugging session.
                        //
                        // Without the XKB extension, the X.Org server sends us the `MappingNotify`
                        // event when there's been a change in the keyboard layout. This stops
                        // being the case when we select ourselves some XKB events with `XkbSelectEvents`
                        // and the "core keyboard device (0x100)" (we haven't tried with any other
                        // devices). We managed to reproduce this on both our machines.
                        //
                        // With the XKB extension active, it would seem like we're supposed to use the
                        // `XkbStateNotify` event to detect keyboard layout changes, but the `group`
                        // never changes value (it is always `0`). This worked for @ArturKovacs, but
                        // not for me. We also tried to use the `group` given to us in keypress events,
                        // but it remained constant there, too.
                        //
                        // We also tried to see if there was some other event that got fired when the
                        // keyboard layout changed, and we found a mysterious event with the value
                        // `85` (`0x55`). We couldn't find any reference to it in the X11 headers or
                        // in the X.Org server source.
                        //
                        // `KeymapNotify` did briefly look interesting based purely on the name, but
                        // it is only useful for checking what keys are pressed when we receive the
                        // event.
                        //
                        // So instead of any vaguely reasonable approach, we get this: reloading the
                        // keymap on *every* keypress. That's peak efficiency right there!
                        //
                        // FIXME: Someone please save our souls! Or at least our wasted CPU cycles.
                        unsafe { self.kb_state.init_with_x11_keymap() };

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
                        let physical_key = keymap::rawkey_to_keycode(scancode as u32);
                        let modifiers = self.device_mod_state.modifiers();

                        callback(Event::DeviceEvent {
                            device_id,
                            event: DeviceEvent::Key(RawKeyEvent {
                                physical_key,
                                state,
                            }),
                        });

                        // `ModifiersChanged` is dispatched here because we assume that every `XI_KeyPress`
                        // is preceeded by a `XI_RawKeyPress`.
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
                if event_type == self.xkbext.first_event_id {
                    let xev = unsafe { &*(identity(xev) as *const _ as *const ffi::XkbAnyEvent) };
                    match xev.xkb_type {
                        ffi::XkbStateNotify => {
                            let xev =
                                unsafe { &*(xev as *const _ as *const ffi::XkbStateNotifyEvent) };
                            if matches!(xev.event_type as i32, ffi::KeyPress | ffi::KeyRelease) {
                                self.kb_state.update_modifiers(
                                    xev.base_mods,
                                    xev.latched_mods,
                                    xev.locked_mods,
                                    xev.base_group as u32,
                                    xev.latched_group as u32,
                                    xev.locked_group as u32,
                                )
                            }
                        }
                        _ => {}
                    }
                }
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
                                                &*window.shared_state.lock(),
                                            );

                                            let window_id = crate::window::WindowId(
                                                crate::platform_impl::platform::WindowId::X(
                                                    *window_id,
                                                ),
                                            );
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
        kb_state: &mut KbState,
        mod_keymap: &ModifierKeymap,
        device_mod_state: &mut ModifierKeyState,
        callback: &mut F,
    ) where
        F: FnMut(Event<'_, T>),
    {
        let device_id = mkdid(util::VIRTUAL_CORE_KEYBOARD);

        // Update modifiers state and emit key events based on which keys are currently pressed.
        for keycode in wt
            .xconn
            .query_keymap()
            .into_iter()
            .filter(|k| *k >= KEYCODE_OFFSET)
        {
            let keycode = (keycode - KEYCODE_OFFSET) as u32;
            let mut ker = kb_state.process_key_event(keycode, state);
            let physical_key = ker.keycode();
            let (logical_key, location) = ker.key();
            let text = ker.text();
            let (key_without_modifiers, _) = ker.key_without_modifiers();
            let text_with_all_modifiers = ker.text_with_all_modifiers();

            if let Some(modifier) = mod_keymap.get_modifier(keycode as ffi::KeyCode) {
                device_mod_state.key_event(
                    ElementState::Pressed,
                    keycode as ffi::KeyCode,
                    modifier,
                );
            }

            callback(Event::WindowEvent {
                window_id,
                event: WindowEvent::KeyboardInput {
                    device_id,
                    event: KeyEvent {
                        physical_key,
                        logical_key,
                        text,
                        location,
                        state,
                        repeat: false,
                        platform_specific: KeyEventExtra {
                            key_without_modifiers,
                            text_with_all_modifiers,
                        },
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
