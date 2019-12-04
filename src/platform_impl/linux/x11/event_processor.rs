use std::{cell::RefCell, collections::HashMap, rc::Rc, slice};

use libc::{c_char, c_int, c_long, c_uint, c_ulong};

use super::{
    events, ffi, get_xtarget, mkdid, mkwid, monitor, util, Device, DeviceId, DeviceInfo, Dnd,
    DndState, GenericEventCookie, ImeReceiver, ScrollOrientation, UnownedWindow, WindowId,
    XExtension,
};

use util::modifiers::{ModifierKeyState, ModifierKeymap};

use crate::{
    dpi::{LogicalPosition, LogicalSize},
    event::{DeviceEvent, ElementState, Event, KeyboardInput, ModifiersState, WindowEvent},
    event_loop::EventLoopWindowTarget as RootELW,
};

pub(super) struct EventProcessor<T: 'static> {
    pub(super) dnd: Dnd,
    pub(super) ime_receiver: ImeReceiver,
    pub(super) randr_event_offset: c_int,
    pub(super) devices: RefCell<HashMap<DeviceId, Device>>,
    pub(super) xi2ext: XExtension,
    pub(super) target: Rc<RootELW<T>>,
    pub(super) mod_keymap: ModifierKeymap,
    pub(super) device_mod_state: ModifierKeyState,
}

impl<T: 'static> EventProcessor<T> {
    pub(super) fn init_device(&self, device: c_int) {
        let wt = get_xtarget(&self.target);
        let mut devices = self.devices.borrow_mut();
        if let Some(info) = DeviceInfo::get(&wt.xconn, device) {
            for info in info.iter() {
                devices.insert(DeviceId(info.deviceid), Device::new(&self, info));
            }
        }
    }

    fn with_window<F, Ret>(&self, window_id: ffi::Window, callback: F) -> Option<Ret>
    where
        F: Fn(&UnownedWindow) -> Ret,
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
            .map(|window| callback(&*window));
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
        F: FnMut(Event<T>),
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
                            let device_id = mkdid(util::VIRTUAL_CORE_KEYBOARD);
                            callback(Event::DeviceEvent {
                                device_id,
                                event: DeviceEvent::ModifiersChanged { modifiers },
                            });
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
                #[derive(Debug, Default)]
                struct Events {
                    resized: Option<WindowEvent>,
                    moved: Option<WindowEvent>,
                    dpi_changed: Option<WindowEvent>,
                }

                let xev: &ffi::XConfigureEvent = xev.as_ref();
                let xwindow = xev.window;
                let events = self.with_window(xwindow, |window| {
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

                    let mut monitor = window.current_monitor(); // This must be done *before* locking!
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

                    let mut events = Events::default();

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
                            let logical_position =
                                LogicalPosition::from_physical(outer, monitor.hidpi_factor);
                            events.moved = Some(WindowEvent::Moved(logical_position));
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
                            .unwrap_or_else(|| (xev.width as f64, xev.height as f64));

                        let last_hidpi_factor = shared_state_lock.last_monitor.hidpi_factor;
                        let new_hidpi_factor = {
                            let window_rect = util::AaRect::new(new_outer_position, new_inner_size);
                            let new_monitor = wt.xconn.get_monitor_for_window(Some(window_rect));

                            if new_monitor.is_dummy() {
                                // Avoid updating monitor using a dummy monitor handle
                                last_hidpi_factor
                            } else {
                                monitor = new_monitor;
                                shared_state_lock.last_monitor = monitor.clone();
                                monitor.hidpi_factor
                            }
                        };
                        if last_hidpi_factor != new_hidpi_factor {
                            events.dpi_changed =
                                Some(WindowEvent::HiDpiFactorChanged(new_hidpi_factor));
                            let (new_width, new_height, flusher) = window.adjust_for_dpi(
                                last_hidpi_factor,
                                new_hidpi_factor,
                                width,
                                height,
                            );
                            flusher.queue();
                            shared_state_lock.dpi_adjusted = Some((new_width, new_height));
                            // if the DPI factor changed, force a resize event to ensure the logical
                            // size is computed with the right DPI factor
                            resized = true;
                        }
                    }

                    // This is a hack to ensure that the DPI adjusted resize is actually applied on all WMs. KWin
                    // doesn't need this, but Xfwm does. The hack should not be run on other WMs, since tiling
                    // WMs constrain the window size, making the resize fail. This would cause an endless stream of
                    // XResizeWindow requests, making Xorg, the winit client, and the WM consume 100% of CPU.
                    if let Some(adjusted_size) = shared_state_lock.dpi_adjusted {
                        let rounded_size = (
                            adjusted_size.0.round() as u32,
                            adjusted_size.1.round() as u32,
                        );
                        if new_inner_size == rounded_size || !util::wm_name_is_one_of(&["Xfwm4"]) {
                            // When this finally happens, the event will not be synthetic.
                            shared_state_lock.dpi_adjusted = None;
                        } else {
                            unsafe {
                                (wt.xconn.xlib.XResizeWindow)(
                                    wt.xconn.display,
                                    xwindow,
                                    rounded_size.0 as c_uint,
                                    rounded_size.1 as c_uint,
                                );
                            }
                        }
                    }

                    if resized {
                        let logical_size =
                            LogicalSize::from_physical(new_inner_size, monitor.hidpi_factor);
                        events.resized = Some(WindowEvent::Resized(logical_size));
                    }

                    events
                });

                if let Some(events) = events {
                    let window_id = mkwid(xwindow);
                    if let Some(event) = events.dpi_changed {
                        callback(Event::WindowEvent { window_id, event });
                    }
                    if let Some(event) = events.resized {
                        callback(Event::WindowEvent { window_id, event });
                    }
                    if let Some(event) = events.moved {
                        callback(Event::WindowEvent { window_id, event });
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

                let window = xev.window;
                let window_id = mkwid(window);

                callback(Event::WindowEvent {
                    window_id,
                    event: WindowEvent::RedrawRequested,
                });
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
                if keycode != 0 {
                    let scancode = keycode - 8;
                    let keysym = wt.xconn.lookup_keysym(xkev);
                    let virtual_keycode = events::keysym_to_element(keysym as c_uint);

                    update_modifiers!(
                        ModifiersState::from_x11_mask(xkev.state),
                        self.mod_keymap.get_modifier(xkev.keycode as ffi::KeyCode)
                    );

                    let modifiers = self.device_mod_state.modifiers();

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

                    for chr in written.chars() {
                        let event = Event::WindowEvent {
                            window_id,
                            event: WindowEvent::ReceivedCharacter(chr),
                        };
                        callback(event);
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
                    Touch, TouchPhase,
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
                                                6 => LineDelta(-1.0, 0.0),
                                                7 => LineDelta(1.0, 0.0),
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
                                    button: Other(x as u8),
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
                            let dpi_factor =
                                self.with_window(xev.event, |window| window.hidpi_factor());
                            if let Some(dpi_factor) = dpi_factor {
                                let position = LogicalPosition::from_physical(
                                    (xev.event_x as f64, xev.event_y as f64),
                                    dpi_factor,
                                );
                                callback(Event::WindowEvent {
                                    window_id,
                                    event: CursorMoved {
                                        device_id,
                                        position,
                                        modifiers,
                                    },
                                });
                            } else {
                                return;
                            }
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
                                                    ScrollOrientation::Horizontal => {
                                                        LineDelta(delta as f32, 0.0)
                                                    }
                                                    // X11 vertical scroll coordinates are opposite to winit's
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
                        callback(Event::WindowEvent {
                            window_id,
                            event: CursorEntered { device_id },
                        });

                        if let Some(dpi_factor) =
                            self.with_window(xev.event, |window| window.hidpi_factor())
                        {
                            let position = LogicalPosition::from_physical(
                                (xev.event_x as f64, xev.event_y as f64),
                                dpi_factor,
                            );

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

                        let dpi_factor =
                            match self.with_window(xev.event, |window| window.hidpi_factor()) {
                                Some(dpi_factor) => dpi_factor,
                                None => return,
                            };
                        let window_id = mkwid(xev.event);

                        wt.ime
                            .borrow_mut()
                            .focus(xev.event)
                            .expect("Failed to focus input context");

                        callback(Event::WindowEvent {
                            window_id,
                            event: Focused(true),
                        });

                        let modifiers = ModifiersState::from_x11(&xev.mods);

                        update_modifiers!(modifiers, None);

                        // The deviceid for this event is for a keyboard instead of a pointer,
                        // so we have to do a little extra work.
                        let pointer_id = self
                            .devices
                            .borrow()
                            .get(&DeviceId(xev.deviceid))
                            .map(|device| device.attachment)
                            .unwrap_or(2);

                        let position = LogicalPosition::from_physical(
                            (xev.event_x as f64, xev.event_y as f64),
                            dpi_factor,
                        );
                        callback(Event::WindowEvent {
                            window_id,
                            event: CursorMoved {
                                device_id: mkdid(pointer_id),
                                position,
                                modifiers,
                            },
                        });

                        // Issue key press events for all pressed keys
                        self.handle_pressed_keys(window_id, ElementState::Pressed, &mut callback);
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

                        let window_id = mkwid(xev.event);

                        // Issue key release events for all pressed keys
                        self.handle_pressed_keys(window_id, ElementState::Released, &mut callback);

                        callback(Event::WindowEvent {
                            window_id,
                            event: Focused(false),
                        })
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
                        let dpi_factor =
                            self.with_window(xev.event, |window| window.hidpi_factor());
                        if let Some(dpi_factor) = dpi_factor {
                            let location = LogicalPosition::from_physical(
                                (xev.event_x as f64, xev.event_y as f64),
                                dpi_factor,
                            );
                            callback(Event::WindowEvent {
                                window_id,
                                event: WindowEvent::Touch(Touch {
                                    device_id: mkdid(xev.deviceid),
                                    phase,
                                    location,
                                    force: None, // TODO
                                    id: xev.detail as u64,
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
                        if keycode < 8 {
                            return;
                        }
                        let scancode = (keycode - 8) as u32;
                        let keysym = wt.xconn.keycode_to_keysym(keycode as ffi::KeyCode);
                        let virtual_keycode = events::keysym_to_element(keysym as c_uint);
                        let modifiers = self.device_mod_state.modifiers();

                        callback(Event::DeviceEvent {
                            device_id,
                            event: DeviceEvent::Key(KeyboardInput {
                                scancode,
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
                                callback(Event::DeviceEvent {
                                    device_id,
                                    event: DeviceEvent::ModifiersChanged {
                                        modifiers: new_modifiers,
                                    },
                                });
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
                            prev_list
                                .iter()
                                .find(|prev_monitor| prev_monitor.name == new_monitor.name)
                                .map(|prev_monitor| {
                                    if new_monitor.hidpi_factor != prev_monitor.hidpi_factor {
                                        for (window_id, window) in wt.windows.borrow().iter() {
                                            if let Some(window) = window.upgrade() {
                                                // Check if the window is on this monitor
                                                let monitor = window.current_monitor();
                                                if monitor.name == new_monitor.name {
                                                    callback(Event::WindowEvent {
                                                        window_id: mkwid(window_id.0),
                                                        event: WindowEvent::HiDpiFactorChanged(
                                                            new_monitor.hidpi_factor,
                                                        ),
                                                    });
                                                    let (width, height) =
                                                        window.inner_size_physical();
                                                    let (_, _, flusher) = window.adjust_for_dpi(
                                                        prev_monitor.hidpi_factor,
                                                        new_monitor.hidpi_factor,
                                                        width as f64,
                                                        height as f64,
                                                    );
                                                    flusher.queue();
                                                }
                                            }
                                        }
                                    }
                                });
                        }
                    }
                }
            }
        }

        match self.ime_receiver.try_recv() {
            Ok((window_id, x, y)) => {
                wt.ime.borrow_mut().send_xim_spot(window_id, x, y);
            }
            Err(_) => (),
        }
    }

    fn handle_pressed_keys<F>(
        &self,
        window_id: crate::window::WindowId,
        state: ElementState,
        callback: &mut F,
    ) where
        F: FnMut(Event<T>),
    {
        let wt = get_xtarget(&self.target);

        let device_id = mkdid(util::VIRTUAL_CORE_KEYBOARD);
        let modifiers = self.device_mod_state.modifiers();

        // Get the set of keys currently pressed and apply Key events to each
        let keys = wt.xconn.query_keymap();

        for keycode in &keys {
            if keycode < 8 {
                continue;
            }

            let scancode = (keycode - 8) as u32;
            let keysym = wt.xconn.keycode_to_keysym(keycode);
            let virtual_keycode = events::keysym_to_element(keysym as c_uint);

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
