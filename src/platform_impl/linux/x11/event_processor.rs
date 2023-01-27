use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};
use x11rb::{
    connection::Connection,
    errors::ConnectionError,
    protocol::{
        xinput::{self, ConnectionExt as _},
        xproto::{self, ConnectionExt as _, Mapping, Window},
        Event as X11Event,
    },
};

use libc::c_uint;

use super::{
    atoms::*,
    events, ffi, fp1616, fp3232, get_xtarget,
    ime::{ImeEvent, ImeRequest},
    mkdid, mkwid, monitor,
    util::{self, PlErrorExt},
    Device, DeviceId, DeviceInfo, Dnd, DndState, ScrollOrientation, UnownedWindow, WindowId,
};

use crate::event::{
    ElementState::{Pressed, Released},
    Ime,
    MouseButton::{Left, Middle, Other, Right},
    MouseScrollDelta::LineDelta,
    Touch,
    WindowEvent::{
        AxisMotion, CursorEntered, CursorLeft, CursorMoved, Focused, MouseInput, MouseWheel,
    },
};

use util::modifiers::{ModifierKeyState, ModifierKeymap};

use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{
        DeviceEvent, ElementState, Event, KeyboardInput, ModifiersState, TouchPhase, WindowEvent,
    },
    event_loop::EventLoopWindowTarget as RootELW,
};

/// The X11 documentation states: "Keycodes lie in the inclusive range `[8, 255]`".
const KEYCODE_OFFSET: u8 = 8;

pub(super) struct EventProcessor<T: 'static> {
    pub(super) dnd: Dnd,
    pub(super) devices: RefCell<HashMap<DeviceId, Device>>,
    pub(super) target: Rc<RootELW<T>>,
    pub(super) mod_keymap: ModifierKeymap,
    pub(super) device_mod_state: ModifierKeyState,
    // Number of touch events currently in progress
    pub(super) num_touch: u32,
    pub(super) first_touch: Option<u64>,
    // Currently focused window belonging to this process
    pub(super) active_window: Option<Window>,
    pub(super) is_composing: bool,
}

impl<T: 'static> EventProcessor<T> {
    pub(super) fn init_device(&self, device: xinput::DeviceId) {
        let wt = get_xtarget(&self.target);
        let mut devices = self.devices.borrow_mut();
        if let Some(info) = DeviceInfo::get(&wt.xconn, device) {
            for info in info.iter() {
                devices.insert(DeviceId(info.deviceid), Device::new(info));
            }
        }
    }

    fn with_window<F, Ret>(&self, window_id: Window, callback: F) -> Option<Ret>
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

    /// See if there are any pending events in the queue.
    ///
    /// Returns `true` if there are events in the queue, `false` otherwise.
    pub(super) fn poll(&mut self) -> bool {
        let wt = get_xtarget(&self.target);
        let mut event_queue = wt.xconn.event_queue.lock().unwrap();

        loop {
            // If we have events in the queue, we need to process them.
            if !event_queue.is_empty() {
                return true;
            }

            // See if the X connection has any events.
            let wt = get_xtarget(&self.target);
            match wt.xconn.connection.poll_for_event() {
                Ok(Some(event)) => {
                    event_queue.push_back(event);
                }
                Ok(None) => {
                    // No events in the queue, and no events on the connection.
                    return false;
                }
                Err(err) => {
                    // An error occurred while polling for events.
                    log::error!("Error while polling for events: {}", err);
                    return false;
                }
            }
        }
    }

    fn window_exists(&self, window_id: Window) -> bool {
        self.with_window(window_id, |_| ()).is_some()
    }

    pub(super) fn poll_one_event(&mut self) -> Result<Option<X11Event>, ConnectionError> {
        let wt = get_xtarget(&self.target);

        // If we previously polled and found an event, return it.
        if let Some(event) = wt.xconn.event_queue.lock().unwrap().pop_front() {
            return Ok(Some(event));
        }

        wt.xconn.connection.poll_for_event()
    }

    pub(super) fn process_event<F>(&mut self, xev: &mut X11Event, mut callback: F)
    where
        F: FnMut(Event<'_, T>),
    {
        let wt = get_xtarget(&self.target);

        // Filter out any IME events.
        if let Some(ime_data) = wt.ime.as_ref() {
            if ime_data.filter_event(xev) {
                return;
            }
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

        let is_synthetic = xev.sent_event();

        match xev {
            X11Event::MappingNotify(mapping) => {
                if matches!(mapping.request, Mapping::MODIFIER | Mapping::KEYBOARD) {
                    // TODO: Use XKB to get more accurate keymap info.

                    // Update modifier keymap.
                    self.mod_keymap
                        .reset_from_x_connection(&wt.xconn)
                        .expect("Failed to update modifier keymap");
                    self.device_mod_state.update_keymap(&self.mod_keymap);
                }
            }

            X11Event::ClientMessage(mut client_msg) => {
                let window = client_msg.window;
                let window_id = mkwid(window);

                let atom = client_msg.data.as_data32()[0] as xproto::Atom;
                if atom == wt.xconn.atoms[WM_DELETE_WINDOW] {
                    callback(Event::WindowEvent {
                        window_id,
                        event: WindowEvent::CloseRequested,
                    });
                } else if atom == wt.xconn.atoms[_NET_WM_PING] {
                    client_msg.window = wt.root;
                    wt.xconn
                        .connection
                        .send_event(
                            false,
                            wt.root,
                            xproto::EventMask::SUBSTRUCTURE_NOTIFY
                                | xproto::EventMask::SUBSTRUCTURE_REDIRECT,
                            client_msg,
                        )
                        .expect("Failed to send ping event")
                        .ignore_error();
                } else if client_msg.type_ == wt.xconn.atoms[XdndEnter] {
                    let longs = client_msg.data.as_data32();
                    let source_window = longs[0] as xproto::Window;
                    let flags = longs[1];
                    let version = flags >> 24;
                    self.dnd.version = Some(version);
                    let has_more_types = flags - (flags & (u32::max_value() - 1)) == 1;
                    if !has_more_types {
                        let type_list = vec![longs[2], longs[3], longs[4]];
                        self.dnd.type_list = Some(type_list);
                    } else if let Ok(more_types) = self.dnd.get_type_list(source_window) {
                        self.dnd.type_list = Some(more_types);
                    }
                } else if client_msg.type_ == wt.xconn.atoms[XdndPosition] {
                    // This event occurs every time the mouse moves while a file's being dragged
                    // over our window. We emit HoveredFile in response; while the macOS backend
                    // does that upon a drag entering, XDND doesn't have access to the actual drop
                    // data until this event. For parity with other platforms, we only emit
                    // `HoveredFile` the first time, though if winit's API is later extended to
                    // supply position updates with `HoveredFile` or another event, implementing
                    // that here would be trivial.

                    let longs = client_msg.data.as_data32();
                    let source_window = longs[0];

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
                        type_list.contains(&wt.xconn.atoms[TextUriList])
                    } else {
                        false
                    };

                    if accepted {
                        self.dnd.source_window = Some(source_window);
                        {
                            if self.dnd.result.is_none() {
                                let time = if version >= 1 {
                                    longs[3]
                                } else {
                                    // In version 0, time isn't specified
                                    0
                                };

                                // This results in the `SelectionNotify` event below
                                self.dnd
                                    .convert_selection(window, time)
                                    .expect("Failed to convert selection");
                            }
                            self.dnd
                                .send_status(window, source_window, DndState::Accepted)
                                .expect("Failed to send `XdndStatus` message.");
                        }
                    } else {
                        {
                            self.dnd
                                .send_status(window, source_window, DndState::Rejected)
                                .expect("Failed to send `XdndStatus` message.");
                        }
                        self.dnd.reset();
                    }
                } else if client_msg.type_ == wt.xconn.atoms[XdndDrop] {
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
                        let source_window = client_msg.data.as_data32()[0];
                        (source_window, DndState::Rejected)
                    };
                    {
                        self.dnd
                            .send_finished(window, source_window, state)
                            .expect("Failed to send `XdndFinished` message.");
                    }
                    self.dnd.reset();
                } else if client_msg.type_ == wt.xconn.atoms[XdndLeave] {
                    self.dnd.reset();
                    callback(Event::WindowEvent {
                        window_id,
                        event: WindowEvent::HoveredFileCancelled,
                    });
                }
            }

            X11Event::SelectionNotify(xsel) => {
                let window = xsel.requestor;
                let window_id = mkwid(window);

                if xsel.property == wt.xconn.atoms[XdndSelection] {
                    let mut result = None;

                    // This is where we receive data from drag and drop
                    if let Ok(mut data) = self.dnd.read_data(window) {
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

            X11Event::ConfigureNotify(xev) => {
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

                    // These are both in physical space.
                    let new_inner_size = (xev.width as u32, xev.height as u32);
                    let new_inner_position = (xev.x.into(), xev.y.into());

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
                        if new_inner_size == adjusted_size
                            || !wt.xconn.wm_name_is_one_of(&["Xfwm4"])
                        {
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

            X11Event::ReparentNotify(xev) => {
                // This is generally a reliable way to detect when the window manager's been
                // replaced, though this event is only fired by reparenting window managers
                // (which is almost all of them). Failing to correctly update WM info doesn't
                // really have much impact, since on the WMs affected (xmonad, dwm, etc.) the only
                // effect is that we waste some time trying to query unsupported properties.
                wt.xconn
                    .update_cached_wm_info(wt.root)
                    .expect("Failed to update WM info");

                self.with_window(xev.window, |window| {
                    window.invalidate_cached_frame_extents();
                });
            }
            X11Event::MapNotify(xev) => {
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
            X11Event::DestroyNotify(xev) => {
                let window = xev.window;
                let window_id = mkwid(window);

                // In the event that the window's been destroyed without being dropped first, we
                // cleanup again here.
                wt.windows.borrow_mut().remove(&WindowId(window as _));

                // Since all XIM stuff needs to happen from the same thread, we destroy the input
                // context here instead of when dropping the window.
                if let Some(ime) = wt.ime.as_ref() {
                    ime.remove_context(window as _)
                        .expect("Failed to destroy input context");
                }

                callback(Event::WindowEvent {
                    window_id,
                    event: WindowEvent::Destroyed,
                });
            }

            X11Event::VisibilityNotify(xev) => {
                let xwindow = xev.window;
                callback(Event::WindowEvent {
                    window_id: mkwid(xwindow),
                    event: WindowEvent::Occluded(xev.state == xproto::Visibility::FULLY_OBSCURED),
                });
                self.with_window(xwindow, |window| {
                    window.visibility_notify();
                });
            }

            X11Event::Expose(xev) => {
                // Multiple Expose events may be received for subareas of a window.
                // We issue `RedrawRequested` only for the last event of such a series.
                if xev.count == 0 {
                    let window = xev.window;
                    let window_id = mkwid(window);

                    callback(Event::RedrawRequested(window_id));
                }
            }

            X11Event::KeyPress(ref xkev) | X11Event::KeyRelease(ref xkev) => {
                // Note that in compose/pre-edit sequences, this will always be Released.
                let state = if matches!(&xev, X11Event::KeyPress(_)) {
                    Pressed
                } else {
                    Released
                };

                let window = xkev.event;
                let window_id = mkwid(window);

                // Standard virtual core keyboard ID. XInput2 needs to be used to get a reliable
                // value, though this should only be an issue under multiseat configurations.
                let device = util::VIRTUAL_CORE_KEYBOARD;
                let device_id = mkdid(device);
                let keycode = xkev.detail;

                // When a compose sequence or IME pre-edit is finished, it ends in a KeyPress with
                // a keycode of 0.
                if keycode != 0 && !self.is_composing {
                    let scancode = (keycode - KEYCODE_OFFSET) as u32;
                    let keysym = wt.xconn.lookup_keysym(xkev);
                    let virtual_keycode = events::keysym_to_element(keysym as c_uint);

                    update_modifiers!(
                        ModifiersState::from_x11_mask(xkev.state.into()),
                        self.mod_keymap.get_modifier(xkev.detail as _)
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

                /*
                TODO: XKB
                if state == Pressed {
                    let written = if let Some(ic) = wt.ime.borrow().get_context(window as _) {
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
                */
            }

            X11Event::XinputButtonPress(ref xbev) | X11Event::XinputButtonRelease(ref xbev) => {
                let window_id = mkwid(xbev.event);
                let device_id = mkdid(xbev.deviceid);

                // Once psychon/x11rb#768 reaches a release, use BitAnd directly on the flags.
                if (u32::from(xbev.flags) & u32::from(xinput::PointerEventFlags::POINTER_EMULATED))
                    != 0
                {
                    // Deliver multi-touch events instead of emulated mouse events.
                    return;
                }

                let modifiers = ModifiersState::from_x11(&xbev.mods);
                update_modifiers!(modifiers, None);

                let state = if matches!(&xev, X11Event::XinputButtonPress(_)) {
                    Pressed
                } else {
                    Released
                };
                match xbev.detail {
                    1 => callback(Event::WindowEvent {
                        window_id,
                        event: MouseInput {
                            device_id,
                            state,
                            button: Left,
                            modifiers,
                        },
                    }),
                    2 => callback(Event::WindowEvent {
                        window_id,
                        event: MouseInput {
                            device_id,
                            state,
                            button: Middle,
                            modifiers,
                        },
                    }),
                    3 => callback(Event::WindowEvent {
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
                        if u32::from(xbev.flags)
                            & u32::from(xinput::PointerEventFlags::POINTER_EMULATED)
                            == 0
                        {
                            callback(Event::WindowEvent {
                                window_id,
                                event: MouseWheel {
                                    device_id,
                                    delta: match xbev.detail {
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

            X11Event::XinputMotion(xev) => {
                let device_id = mkdid(xev.deviceid);
                let window_id = mkwid(xev.event);
                let new_cursor_pos = (fp1616(xev.event_x), fp1616(xev.event_y));

                let modifiers = ModifiersState::from_x11(&xev.mods);
                update_modifiers!(modifiers, None);

                let cursor_moved = self.with_window(xev.event, |window| {
                    let mut shared_state_lock = window.shared_state_lock();
                    util::maybe_change(&mut shared_state_lock.cursor_pos, new_cursor_pos)
                });
                if cursor_moved == Some(true) {
                    let position = PhysicalPosition::new(xev.event_x as f64, xev.event_y as f64);

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
                    let mut devices = self.devices.borrow_mut();
                    let physical_device = match devices.get_mut(&DeviceId(xev.sourceid)) {
                        Some(device) => device,
                        None => return,
                    };

                    // Iterator over the set bits in the mask.
                    let bits_iter = xev.valuator_mask.iter().enumerate().flat_map(|(i, &mask)| {
                        let quantum = std::mem::size_of::<u32>();
                        (0..quantum)
                            .filter(move |j| mask & (1 << j) != 0)
                            .map(move |j| i * quantum + j)
                    });

                    // Get the iterator over the axises that we want.
                    let axis_iter = xev
                        .axisvalues
                        .iter()
                        .map(|&frac| fp3232(frac))
                        .zip(bits_iter);

                    // Iterate and set the axises.
                    axis_iter.for_each(|(x, i)| {
                        if let Some(&mut (_, ref mut info)) = physical_device
                            .scroll_axes
                            .iter_mut()
                            .find(|&&mut (axis, _)| axis as usize == i)
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
                                    value: x,
                                },
                            });
                        }
                    });
                }
                for event in events {
                    callback(event);
                }
            }

            X11Event::XinputEnter(xev) => {
                let window_id = mkwid(xev.event);
                let device_id = mkdid(xev.deviceid);

                if let Some(all_info) = DeviceInfo::get(&wt.xconn, 0) {
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

                    let position = PhysicalPosition::new(fp1616(xev.event_x), fp1616(xev.event_y));

                    // The mods field on this event isn't actually populated, so query the
                    // pointer device. In the future, we can likely remove this round-trip by
                    // relying on `Xkb` for modifier values.
                    //
                    // This needs to only be done after confirming the window still exists,
                    // since otherwise we risk getting a `BadWindow` error if the window was
                    // dropped with queued events.
                    let modifiers = wt
                        .xconn
                        .connection
                        .xinput_xi_query_pointer(xev.event, xev.deviceid)
                        .platform()
                        .and_then(|r| r.reply().platform())
                        .map(|r| ModifiersState::from_x11(&r.mods))
                        .expect("Failed to query pointer device");

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

            X11Event::XinputLeave(xev) => {
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

            X11Event::XinputFocusIn(xev) => {
                if let Some(ime) = wt.ime.as_ref() {
                    ime.focus_window(&wt.xconn, xev.event as _)
                        .expect("Failed to focus input context");
                }

                let modifiers = ModifiersState::from_x11(&xev.mods);

                self.device_mod_state.update_state(&modifiers, None);

                if self.active_window != Some(xev.event) {
                    self.active_window = Some(xev.event);

                    wt.update_device_event_filter(true)
                        .expect("Failed to update device event filter");

                    let window_id = mkwid(xev.event);
                    let position = PhysicalPosition::new(fp1616(xev.event_x), fp1616(xev.event_y));

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

            X11Event::XinputFocusOut(xev) => {
                if !self.window_exists(xev.event) {
                    return;
                }

                if let Some(ime) = wt.ime.as_ref() {
                    ime.unfocus_window(&wt.xconn, xev.event as _)
                        .expect("Failed to focus input context");
                }

                if self.active_window.take() == Some(xev.event) {
                    let window_id = mkwid(xev.event);

                    wt.update_device_event_filter(false)
                        .expect("Failed to update device event filter");

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

            X11Event::XinputTouchBegin(ref xtev)
            | X11Event::XinputTouchEnd(ref xtev)
            | X11Event::XinputTouchUpdate(ref xtev) => {
                let window_id = mkwid(xtev.event);
                let phase = match xev {
                    X11Event::XinputTouchBegin(_) => TouchPhase::Started,
                    X11Event::XinputTouchUpdate(_) => TouchPhase::Moved,
                    X11Event::XinputTouchEnd(_) => TouchPhase::Ended,
                    _ => unreachable!(),
                };
                if self.window_exists(xtev.event) {
                    let id = xtev.detail as u64;
                    let modifiers = self.device_mod_state.modifiers();
                    let location = PhysicalPosition::new(xtev.event_x as f64, xtev.event_y as f64);

                    // Mouse cursor position changes when touch events are received.
                    // Only the first concurrently active touch ID moves the mouse cursor.
                    if is_first_touch(&mut self.first_touch, &mut self.num_touch, id, phase) {
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
                            device_id: mkdid(xtev.deviceid),
                            phase,
                            location,
                            force: None, // TODO
                            id,
                        }),
                    })
                }
            }

            X11Event::XinputRawButtonPress(ref xbev)
            | X11Event::XinputRawButtonRelease(ref xbev) => {
                if u32::from(xbev.flags) & u32::from(xinput::PointerEventFlags::POINTER_EMULATED)
                    == 0
                {
                    callback(Event::DeviceEvent {
                        device_id: mkdid(xbev.deviceid),
                        event: DeviceEvent::Button {
                            button: xbev.detail,
                            state: match xev {
                                X11Event::XinputRawButtonPress(_) => Pressed,
                                X11Event::XinputRawButtonRelease(_) => Released,
                                _ => unreachable!(),
                            },
                        },
                    });
                }
            }

            X11Event::XinputRawMotion(xev) => {
                let did = mkdid(xev.deviceid);

                let mut mouse_delta = (0.0, 0.0);
                let mut scroll_delta = (0.0, 0.0);

                // Iterate over all bits in the mask.
                let bits_iter = xev.valuator_mask.iter().enumerate().flat_map(|(i, &mask)| {
                    let quantum = std::mem::size_of::<u32>();

                    (0..quantum * 8).filter_map(move |j| {
                        let bit = 1 << j;
                        if mask & bit != 0 {
                            Some(i * quantum * 8 + j)
                        } else {
                            None
                        }
                    })
                });

                // Match those bits to the raw values.
                let values_iter = xev.axisvalues_raw.iter().map(|&v| fp3232(v)).zip(bits_iter);

                values_iter.for_each(|(x, i)| {
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
                });
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

            X11Event::XinputRawKeyPress(ref xkev) | X11Event::XinputRawKeyRelease(ref xkev) => {
                let state = match xev {
                    X11Event::XinputRawKeyPress(_) => Pressed,
                    X11Event::XinputRawKeyRelease(_) => Released,
                    _ => unreachable!(),
                };

                let device_id = mkdid(xkev.sourceid);
                let keycode = xkev.detail;
                let scancode = match keycode.checked_sub(KEYCODE_OFFSET as u32) {
                    Some(scancode) => scancode,
                    None => return,
                };
                let keysym = wt.xconn.keycode_to_keysym(keycode as ffi::KeyCode);
                let virtual_keycode = events::keysym_to_element(keysym as c_uint);
                let modifiers = self.device_mod_state.modifiers();

                #[allow(deprecated)]
                callback(Event::DeviceEvent {
                    device_id,
                    event: DeviceEvent::Key(KeyboardInput {
                        scancode,
                        virtual_keycode,
                        state,
                        modifiers,
                    }),
                });

                if let Some(modifier) = self.mod_keymap.get_modifier(keycode as ffi::KeyCode) {
                    self.device_mod_state
                        .key_event(state, keycode as ffi::KeyCode, modifier);

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

            X11Event::XinputHierarchy(xev) => {
                for info in xev.infos.iter() {
                    if 0 != (u32::from(info.flags)
                        & u32::from(
                            xinput::HierarchyMask::SLAVE_ADDED
                                | xinput::HierarchyMask::MASTER_ADDED,
                        ))
                    {
                        self.init_device(info.deviceid);
                        callback(Event::DeviceEvent {
                            device_id: mkdid(info.deviceid),
                            event: DeviceEvent::Added,
                        });
                    } else if 0
                        != (u32::from(info.flags)
                            & u32::from(
                                xinput::HierarchyMask::SLAVE_REMOVED
                                    | xinput::HierarchyMask::MASTER_REMOVED,
                            ))
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

            X11Event::RandrNotify(_) => {
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
                                            maybe_prev_scale_factor.unwrap_or(monitor.scale_factor),
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
                                            window.set_inner_size_physical(new_width, new_height);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        // Handle IME requests.
        if let Some(ime) = wt.ime.as_ref() {
            if let Ok(ime_request) = wt.ime_receiver.try_recv() {
                match ime_request {
                    ImeRequest::Position(window_id, x, y) => {
                        ime.set_spot(&wt.xconn, window_id, x, y)
                            .expect("Failed to set IME spot");
                    }
                    ImeRequest::Allow(window_id, allowed) => {
                        ime.set_ime_allowed(&wt.xconn, window_id, allowed)
                            .expect("Failed to set IME allowed");
                    }
                }
            }

            // See if any IME events have occurred.
            let (window, event) = match ime.next_event() {
                Some((window, event)) => (window, event),
                None => return,
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
