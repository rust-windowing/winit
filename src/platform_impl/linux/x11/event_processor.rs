use std::sync::atomic::Ordering::Relaxed;
use std::{collections::HashMap, rc::Rc, slice, sync::Arc};

use parking_lot::MutexGuard;
use SeatFocus::{KbFocus, PtrFocus};

use super::{
    ffi, get_xtarget, mkdid, mkwid, util, Device, DeviceId, DeviceInfo, Dnd, DndState,
    ScrollOrientation, UnownedWindow, WindowId,
};

use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{DeviceEvent, Event, KeyEvent, RawKeyEvent, TouchPhase, WindowEvent},
    event_loop::EventLoopWindowTarget as RootELW,
    keyboard::ModifiersState,
    platform_impl::platform::{
        common::{keymap, xkb_state::KbState},
        KeyEventExtra,
    },
    platform_impl::x11::EventLoopWindowTarget,
};

/// The X11 documentation states: "Keycodes lie in the inclusive range [8,255]".
const KEYCODE_OFFSET: u8 = 8;

pub(super) struct Seat {
    kb_state: KbState,
    /// The master keyboard of this seat
    keyboard: ffi::xcb_input_device_id_t,
    /// The master pointer of this seat
    pointer: ffi::xcb_input_device_id_t,
    /// The window that has this seats keyboard focus
    kb_focus: Option<ffi::xcb_window_t>,
    /// The window that has this seats pointer focus
    ptr_focus: Option<ffi::xcb_window_t>,
    /// The latest modifiers state
    current_modifiers: ModifiersState,

    num_errors: usize,
}

enum SeatFocus {
    KbFocus,
    PtrFocus,
}

pub(super) struct EventProcessor<T: 'static> {
    pub(super) dnd: Dnd,
    pub(super) devices: HashMap<DeviceId, Device>,
    pub(super) target: Rc<RootELW<T>>,
    pub(super) seats: Vec<Seat>,
    // Number of touch events currently in progress
    pub(super) num_touch: u32,
    pub(super) first_touch: Option<u64>,
}

impl<T: 'static> EventProcessor<T> {
    pub(super) fn init_device(
        target: &RootELW<T>,
        devices: &mut HashMap<DeviceId, Device>,
        seats: &mut Vec<Seat>,
        device: ffi::xcb_input_device_id_t,
    ) {
        let wt = get_xtarget(target);
        if let Some(info) = DeviceInfo::get(&wt.xconn, device) {
            for info in info {
                let info = unsafe { &*info };
                let device_id = DeviceId(info.deviceid);
                if info.type_ == ffi::XCB_INPUT_DEVICE_TYPE_MASTER_KEYBOARD as _ {
                    if devices.contains_key(&device_id) {
                        seats.retain(|s| s.keyboard != info.deviceid);
                    }
                    match wt.xconn.make_auto_repeat_detectable(device_id.0) {
                        Ok(true) => {}
                        Ok(false) => log::warn!("X server does not support detectable auto-repeat"),
                        Err(e) => log::error!(
                            "Could not enable detectable auto repeat for device {}: {}",
                            device_id.0,
                            e
                        ),
                    }
                    let kb_state = KbState::from_x11_xkb(wt.xconn.c, info.deviceid).unwrap();
                    seats.push(Seat {
                        kb_state,
                        keyboard: info.deviceid,
                        pointer: info.attachment,
                        kb_focus: None,
                        ptr_focus: None,
                        current_modifiers: ModifiersState::empty(),
                        num_errors: 0,
                    });
                }
                devices.insert(device_id, Device::new(wt, info));
            }
        }
    }

    fn with_window<F, Ret>(
        wt: &EventLoopWindowTarget<T>,
        window_id: ffi::xcb_window_t,
        callback: F,
    ) -> Option<Ret>
    where
        F: Fn(&Arc<UnownedWindow>) -> Ret,
    {
        let mut deleted = false;
        let window_id = WindowId(window_id);
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

    fn window_exists(wt: &EventLoopWindowTarget<T>, window_id: ffi::xcb_window_t) -> bool {
        Self::with_window(wt, window_id, |_| ()).is_some()
    }

    pub(super) fn process_event<F>(&mut self, xev: &mut ffi::xcb_generic_event_t, mut callback: F)
    where
        F: FnMut(Event<'_, T>),
    {
        let wt = get_xtarget(&self.target);
        {
            let reset_dead_keys = wt.reset_dead_keys.load(Relaxed);
            if reset_dead_keys != 0 {
                for seat in &mut self.seats {
                    seat.kb_state.reset_dead_keys();
                }
                wt.reset_dead_keys.fetch_sub(reset_dead_keys, Relaxed);
            }
        }

        let response_type = xev.response_type & 0x7f;
        match response_type {
            0 => unsafe {
                let error = &*(xev as *const _ as *const ffi::xcb_generic_error_t);
                let error = wt.xconn.errors.parse(error);
                log::error!("An unchecked error occurred: {}", error);
            },

            ffi::XCB_CLIENT_MESSAGE => {
                let client_msg =
                    unsafe { &*(xev as *const _ as *const ffi::xcb_client_message_event_t) };
                let data32 = unsafe { client_msg.data.data32 };

                let window = client_msg.window;
                let window_id = mkwid(window);

                if data32[0] == wt.wm_delete_window {
                    callback(Event::WindowEvent {
                        window_id,
                        event: WindowEvent::CloseRequested,
                    });
                } else if data32[0] == wt.net_wm_ping {
                    Self::with_window(wt, window, |w| {
                        let response_msg = ffi::xcb_client_message_event_t {
                            window: w.screen.root,
                            ..*client_msg
                        };
                        let pending = wt.xconn.send_event(
                            w.screen.root,
                            Some(
                                ffi::XCB_EVENT_MASK_SUBSTRUCTURE_NOTIFY
                                    | ffi::XCB_EVENT_MASK_SUBSTRUCTURE_REDIRECT,
                            ),
                            &response_msg,
                        );
                        wt.xconn.discard(pending);
                    });
                } else if client_msg.type_ == self.dnd.atoms.enter {
                    let source_window = data32[0];
                    let flags = data32[1];
                    let version = flags >> 24;
                    self.dnd.version = Some(version);
                    let has_more_types = flags - (flags & (u32::MAX - 1)) == 1;
                    if !has_more_types {
                        let type_list = vec![data32[2], data32[3], data32[4]];
                        self.dnd.type_list = Some(type_list);
                    } else if let Ok(more_types) = unsafe { self.dnd.get_type_list(source_window) }
                    {
                        self.dnd.type_list = Some(more_types);
                    }
                } else if client_msg.type_ == self.dnd.atoms.position {
                    // This event occurs every time the mouse moves while a file's being dragged
                    // over our window. We emit HoveredFile in response; while the macOS backend
                    // does that upon a drag entering, XDND doesn't have access to the actual drop
                    // data until this event. For parity with other platforms, we only emit
                    // `HoveredFile` the first time, though if winit's API is later extended to
                    // supply position updates with `HoveredFile` or another event, implementing
                    // that here would be trivial.

                    let source_window = data32[0];

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
                                    data32[3]
                                } else {
                                    // In version 0, time isn't specified
                                    ffi::XCB_TIME_CURRENT_TIME
                                };
                                // This results in the `SelectionNotify` event below
                                self.dnd.convert_selection(window, time);
                            }
                            self.dnd
                                .send_status(window, source_window, DndState::Accepted);
                        }
                    } else {
                        unsafe {
                            self.dnd
                                .send_status(window, source_window, DndState::Rejected);
                        }
                        self.dnd.reset();
                    }
                } else if client_msg.type_ == self.dnd.atoms.drop {
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
                        let source_window = data32[0];
                        (source_window, DndState::Rejected)
                    };
                    unsafe {
                        self.dnd
                            .send_finished(window, source_window, state)
                            .expect("Failed to send `XdndFinished` message.");
                    }
                    self.dnd.reset();
                } else if client_msg.type_ == self.dnd.atoms.leave {
                    self.dnd.reset();
                    callback(Event::WindowEvent {
                        window_id,
                        event: WindowEvent::HoveredFileCancelled,
                    });
                }
            }

            ffi::XCB_SELECTION_NOTIFY => {
                let xsel =
                    unsafe { &*(xev as *const _ as *const ffi::xcb_selection_notify_event_t) };

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

            ffi::XCB_CONFIGURE_NOTIFY => {
                let xev =
                    unsafe { &*(xev as *const _ as *const ffi::xcb_configure_notify_event_t) };
                let xwindow = xev.window;
                let window_id = mkwid(xwindow);

                if let Some(window) = Self::with_window(wt, xwindow, Arc::clone) {
                    // So apparently...
                    // `XSendEvent` (synthetic `ConfigureNotify`) -> position relative to root
                    // `XConfigureNotify` (real `ConfigureNotify`) -> position relative to parent
                    // https://tronche.com/gui/x/icccm/sec-4.html#s-4.1.5
                    // We don't want to send `Moved` when this is false, since then every `Resized`
                    // (whether the window moved or not) is accompanied by an extraneous `Moved` event
                    // that has a position relative to the parent window.
                    let is_synthetic = xev.response_type & 0x80 != 0;

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
                                let frame_extents = wt.xconn.get_frame_extents_heuristic(&window);
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
                            .unwrap_or_else(|| (xev.width as u32, xev.height as u32));

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
                        if new_inner_size == adjusted_size
                            || !window.screen.wm_name_is_one_of(&["Xfwm4"])
                        {
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

            ffi::XCB_REPARENT_NOTIFY => {
                let xev = unsafe { &*(xev as *const _ as *const ffi::xcb_reparent_notify_event_t) };

                // This is generally a reliable way to detect when the window manager's been
                // replaced, though this event is only fired by reparenting window managers
                // (which is almost all of them). Failing to correctly update WM info doesn't
                // really have much impact, since on the WMs affected (xmonad, dwm, etc.) the only
                // effect is that we waste some time trying to query unsupported properties.
                wt.xconn.update_cached_wm_info();

                Self::with_window(wt, xev.window, |window| {
                    window.invalidate_cached_frame_extents();
                });
            }

            ffi::XCB_DESTROY_NOTIFY => {
                let xev = unsafe { &*(xev as *const _ as *const ffi::xcb_destroy_notify_event_t) };

                let window = xev.window;
                let window_id = mkwid(window);

                // In the event that the window's been destroyed without being dropped first, we
                // cleanup again here.
                wt.windows.borrow_mut().remove(&WindowId(window));

                callback(Event::WindowEvent {
                    window_id,
                    event: WindowEvent::Destroyed,
                });
            }

            ffi::XCB_VISIBILITY_NOTIFY => {
                let xev =
                    unsafe { &*(xev as *const _ as *const ffi::xcb_visibility_notify_event_t) };
                let xwindow = xev.window;

                Self::with_window(wt, xwindow, |window| window.visibility_notify());
            }

            ffi::XCB_EXPOSE => {
                let xev = unsafe { &*(xev as *const _ as *const ffi::xcb_expose_event_t) };

                // Multiple Expose events may be received for subareas of a window.
                // We issue `RedrawRequested` only for the last event of such a series.
                if xev.count == 0 {
                    let window = xev.window;
                    let window_id = mkwid(window);

                    callback(Event::RedrawRequested(window_id));
                }
            }

            ffi::XCB_GE_GENERIC => {
                let xev = unsafe { &*(xev as *const _ as *const ffi::xcb_ge_generic_event_t) };

                if wt.xconn.xinput_extension != xev.extension {
                    return;
                }

                use crate::event::{
                    ElementState::{Pressed, Released},
                    MouseButton::{Left, Middle, Other, Right},
                    MouseScrollDelta::LineDelta,
                    Touch,
                    WindowEvent::{AxisMotion, CursorMoved, MouseInput, MouseWheel},
                };

                match xev.event_type {
                    ffi::XCB_INPUT_BUTTON_PRESS | ffi::XCB_INPUT_BUTTON_RELEASE => {
                        let xev = unsafe {
                            &*(xev as *const _ as *const ffi::xcb_input_button_press_event_t)
                        };
                        if (xev.flags & ffi::XCB_INPUT_POINTER_EVENT_FLAGS_POINTER_EMULATED) != 0 {
                            // Deliver multi-touch events instead of emulated mouse events.
                            return;
                        }

                        let seat = match find_seat_by_pointer(&mut self.seats, xev.deviceid) {
                            Some(seat) => seat,
                            _ => return,
                        };
                        Self::update_seat_kb_xi(seat, &xev.mods, &xev.group, &mut callback);
                        Self::update_seat_focus(seat, PtrFocus, wt, Some(xev.event), &mut callback);

                        let window_id = mkwid(xev.event);
                        let device_id = mkdid(seat.keyboard);

                        let state = if xev.event_type == ffi::XCB_INPUT_BUTTON_PRESS {
                            Pressed
                        } else {
                            Released
                        };
                        match xev.detail as u32 {
                            ffi::XCB_BUTTON_INDEX_1
                            | ffi::XCB_BUTTON_INDEX_2
                            | ffi::XCB_BUTTON_INDEX_3 => {
                                let button = match xev.detail as u32 {
                                    ffi::XCB_BUTTON_INDEX_1 => Left,
                                    ffi::XCB_BUTTON_INDEX_2 => Middle,
                                    ffi::XCB_BUTTON_INDEX_3 => Right,
                                    _ => unreachable!(),
                                };
                                callback(Event::WindowEvent {
                                    window_id,
                                    event: MouseInput {
                                        device_id,
                                        state,
                                        button,
                                        modifiers: seat.current_modifiers,
                                    },
                                })
                            }

                            // Suppress emulated scroll wheel clicks, since we handle the real motion events for those.
                            // In practice, even clicky scroll wheels appear to be reported by evdev (and XInput2 in
                            // turn) as axis motion, so we don't otherwise special-case these button presses.
                            4 | 5 | 6 | 7 => {
                                if xev.flags & ffi::XCB_INPUT_POINTER_EVENT_FLAGS_POINTER_EMULATED
                                    == 0
                                {
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
                                            modifiers: seat.current_modifiers,
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
                                    modifiers: seat.current_modifiers,
                                },
                            }),
                        }
                    }
                    ffi::XCB_INPUT_MOTION => {
                        let xev =
                            unsafe { &*(xev as *const _ as *const ffi::xcb_input_motion_event_t) };

                        let seat = match find_seat_by_pointer(&mut self.seats, xev.deviceid) {
                            Some(seat) => seat,
                            _ => return,
                        };
                        Self::update_seat_kb_xi(seat, &xev.mods, &xev.group, &mut callback);
                        Self::update_seat_focus(seat, PtrFocus, wt, Some(xev.event), &mut callback);

                        let device_id = mkdid(seat.keyboard);
                        let window_id = mkwid(xev.event);
                        let event_x = util::fp1616_to_f64(xev.event_x);
                        let event_y = util::fp1616_to_f64(xev.event_y);
                        let new_cursor_pos = (event_x, event_y);

                        let cursor_moved = Self::with_window(wt, xev.event, |window| {
                            let mut shared_state_lock = window.shared_state.lock();
                            util::maybe_change(&mut shared_state_lock.cursor_pos, new_cursor_pos)
                        });
                        if cursor_moved == Some(true) {
                            let position = PhysicalPosition::new(event_x, event_y);

                            callback(Event::WindowEvent {
                                window_id,
                                event: CursorMoved {
                                    device_id,
                                    position,
                                    modifiers: seat.current_modifiers,
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
                                    wt.xconn.xinput.xcb_input_button_press_valuator_mask(xev),
                                    xev.valuators_len as usize,
                                )
                            };
                            let axis_ids = mask_to_axis_ids(mask);
                            let axes = unsafe {
                                slice::from_raw_parts(
                                    wt.xconn.xinput.xcb_input_button_press_axisvalues(xev),
                                    axis_ids.len(),
                                )
                            };
                            let physical_device =
                                match self.devices.get_mut(&DeviceId(xev.sourceid)) {
                                    Some(device) => device,
                                    None => return,
                                };

                            for (&i, &x) in axis_ids.iter().zip(axes.iter()) {
                                let x = util::fp3232_to_f64(x);
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
                                            modifiers: seat.current_modifiers,
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
                            }
                        }
                        for event in events {
                            callback(event);
                        }
                    }

                    ffi::XCB_INPUT_ENTER => {
                        let xev =
                            unsafe { &*(xev as *const _ as *const ffi::xcb_input_enter_event_t) };

                        let seat = match find_seat_by_pointer(&mut self.seats, xev.deviceid) {
                            Some(seat) => seat,
                            _ => return,
                        };
                        Self::update_seat_kb_xi(seat, &xev.mods, &xev.group, &mut callback);
                        Self::update_seat_focus(seat, PtrFocus, wt, Some(xev.event), &mut callback);

                        let device_id = mkdid(seat.keyboard);

                        if let Some(all_info) =
                            DeviceInfo::get(&wt.xconn, ffi::XCB_INPUT_DEVICE_ALL as _)
                        {
                            for device_info in all_info {
                                let device_info = unsafe { &*device_info };
                                if device_info.deviceid == xev.sourceid
                                // This is needed for resetting to work correctly on i3, and
                                // presumably some other WMs. On those, `XI_Enter` doesn't include
                                // the physical device ID, so both `sourceid` and `deviceid` are
                                // the virtual device.
                                || device_info.attachment == xev.sourceid
                                {
                                    let device_id = DeviceId(device_info.deviceid);
                                    if let Some(device) = self.devices.get_mut(&device_id) {
                                        device.reset_scroll_position(wt, device_info);
                                    }
                                }
                            }
                        }

                        if let Some(window) = seat.ptr_focus {
                            let event_x = util::fp1616_to_f64(xev.event_x);
                            let event_y = util::fp1616_to_f64(xev.event_y);
                            let position = PhysicalPosition::new(event_x, event_y);

                            callback(Event::WindowEvent {
                                window_id: mkwid(window),
                                event: CursorMoved {
                                    device_id,
                                    position,
                                    modifiers: seat.current_modifiers,
                                },
                            });
                        }
                    }
                    ffi::XCB_INPUT_LEAVE => {
                        let xev =
                            unsafe { &*(xev as *const _ as *const ffi::xcb_input_leave_event_t) };

                        let seat = match find_seat_by_pointer(&mut self.seats, xev.deviceid) {
                            Some(seat) => seat,
                            _ => return,
                        };
                        Self::update_seat_kb_xi(seat, &xev.mods, &xev.group, &mut callback);
                        Self::update_seat_focus(seat, PtrFocus, wt, None, &mut callback);
                    }
                    ffi::XCB_INPUT_FOCUS_IN => {
                        let xev = unsafe {
                            &*(xev as *const _ as *const ffi::xcb_input_focus_in_event_t)
                        };

                        let seat = match find_seat(&mut self.seats, xev.deviceid) {
                            Some(seat) => seat,
                            _ => return,
                        };
                        Self::update_seat_kb_xi(seat, &xev.mods, &xev.group, &mut callback);
                        Self::update_seat_focus(seat, KbFocus, wt, Some(xev.event), &mut callback);
                    }
                    ffi::XCB_INPUT_FOCUS_OUT => {
                        let xev = unsafe {
                            &*(xev as *const _ as *const ffi::xcb_input_focus_out_event_t)
                        };

                        let seat = match find_seat(&mut self.seats, xev.deviceid) {
                            Some(seat) => seat,
                            _ => return,
                        };
                        Self::update_seat_kb_xi(seat, &xev.mods, &xev.group, &mut callback);
                        Self::update_seat_focus(seat, KbFocus, wt, None, &mut callback);
                    }

                    ffi::XCB_INPUT_TOUCH_BEGIN
                    | ffi::XCB_INPUT_TOUCH_UPDATE
                    | ffi::XCB_INPUT_TOUCH_END => {
                        let xev = unsafe {
                            &*(xev as *const _ as *const ffi::xcb_input_touch_begin_event_t)
                        };
                        let phase = match xev.event_type {
                            ffi::XCB_INPUT_TOUCH_BEGIN => TouchPhase::Started,
                            ffi::XCB_INPUT_TOUCH_UPDATE => TouchPhase::Moved,
                            ffi::XCB_INPUT_TOUCH_END => TouchPhase::Ended,
                            _ => unreachable!(),
                        };
                        let seat = match find_seat_by_pointer(&mut self.seats, xev.deviceid) {
                            Some(seat) => seat,
                            _ => return,
                        };
                        Self::update_seat_kb_xi(seat, &xev.mods, &xev.group, &mut callback);
                        Self::update_seat_focus(seat, PtrFocus, wt, Some(xev.event), &mut callback);

                        if let Some(window) = seat.ptr_focus {
                            let window_id = mkwid(window);
                            let id = xev.detail as u64;
                            let event_x = util::fp1616_to_f64(xev.event_x);
                            let event_y = util::fp1616_to_f64(xev.event_y);
                            let location = PhysicalPosition::new(event_x, event_y);

                            // Mouse cursor position changes when touch events are received.
                            // Only the first concurrently active touch ID moves the mouse cursor.
                            if is_first_touch(&mut self.first_touch, &mut self.num_touch, id, phase)
                            {
                                callback(Event::WindowEvent {
                                    window_id,
                                    event: WindowEvent::CursorMoved {
                                        device_id: mkdid(seat.keyboard),
                                        position: location.cast(),
                                        modifiers: seat.current_modifiers,
                                    },
                                });
                            }

                            callback(Event::WindowEvent {
                                window_id,
                                event: WindowEvent::Touch(Touch {
                                    device_id: mkdid(seat.keyboard),
                                    phase,
                                    location,
                                    force: None, // TODO
                                    id,
                                }),
                            })
                        }
                    }

                    ffi::XCB_INPUT_RAW_BUTTON_PRESS | ffi::XCB_INPUT_RAW_BUTTON_RELEASE => {
                        let xev = unsafe {
                            &*(xev as *const _ as *const ffi::xcb_input_raw_button_press_event_t)
                        };
                        if xev.flags & ffi::XCB_INPUT_POINTER_EVENT_FLAGS_POINTER_EMULATED == 0 {
                            callback(Event::DeviceEvent {
                                device_id: mkdid(xev.deviceid),
                                event: DeviceEvent::Button {
                                    button: xev.detail as u32,
                                    state: match xev.event_type {
                                        ffi::XCB_INPUT_RAW_BUTTON_PRESS => Pressed,
                                        ffi::XCB_INPUT_RAW_BUTTON_RELEASE => Released,
                                        _ => unreachable!(),
                                    },
                                },
                            });
                        }
                    }

                    ffi::XCB_INPUT_RAW_MOTION => {
                        let xev = unsafe {
                            &*(xev as *const _ as *const ffi::xcb_input_raw_motion_event_t)
                        };
                        let did = mkdid(xev.deviceid);

                        let mask = unsafe {
                            slice::from_raw_parts(
                                wt.xconn
                                    .xinput
                                    .xcb_input_raw_button_press_valuator_mask(xev),
                                xev.valuators_len as usize,
                            )
                        };
                        let axis_ids = mask_to_axis_ids(mask);
                        let axes = unsafe {
                            slice::from_raw_parts(
                                wt.xconn.xinput.xcb_input_raw_button_press_axisvalues(xev),
                                axis_ids.len(),
                            )
                        };
                        let mut mouse_delta = (0.0, 0.0);
                        let mut scroll_delta = (0.0, 0.0);
                        for (&i, &x) in axis_ids.iter().zip(axes.iter()) {
                            let x = util::fp3232_to_f64(x);
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
                    ffi::XCB_INPUT_KEY_PRESS | ffi::XCB_INPUT_KEY_RELEASE => {
                        let xkev = unsafe {
                            &*(xev as *const _ as *const ffi::xcb_input_key_press_event_t)
                        };

                        let seat = match find_seat(&mut self.seats, xkev.deviceid) {
                            Some(seat) => seat,
                            _ => return,
                        };
                        Self::update_seat_kb_xi(seat, &xkev.mods, &xkev.group, &mut callback);
                        Self::update_seat_focus(seat, KbFocus, wt, Some(xkev.event), &mut callback);

                        if let Some(focus) = seat.kb_focus {
                            let state = if xev.event_type == ffi::XCB_INPUT_KEY_PRESS {
                                Pressed
                            } else {
                                Released
                            };

                            let device_id = mkdid(seat.keyboard);
                            let keycode = xkev.detail as u32;

                            let ker = seat.kb_state.process_key_event(
                                keycode,
                                xkev.group.effective as u32,
                                state,
                            );
                            let repeat =
                                xkev.flags & ffi::XCB_INPUT_KEY_EVENT_FLAGS_KEY_REPEAT != 0;

                            callback(Event::WindowEvent {
                                window_id: mkwid(focus),
                                event: WindowEvent::KeyboardInput {
                                    device_id,
                                    event: KeyEvent {
                                        physical_key: ker.keycode,
                                        logical_key: ker.key,
                                        text: ker.text,
                                        location: ker.location,
                                        state,
                                        repeat,
                                        platform_specific: KeyEventExtra {
                                            key_without_modifiers: ker.key_without_modifiers,
                                            text_with_all_modifiers: ker.text_with_all_modifiers,
                                        },
                                    },
                                    is_synthetic: false,
                                },
                            });
                        }
                    }

                    ffi::XCB_INPUT_RAW_KEY_PRESS | ffi::XCB_INPUT_RAW_KEY_RELEASE => {
                        let xev = unsafe {
                            &*(xev as *const _ as *const ffi::xcb_input_raw_key_press_event_t)
                        };

                        let state = match xev.event_type {
                            ffi::XCB_INPUT_RAW_KEY_PRESS => Pressed,
                            ffi::XCB_INPUT_RAW_KEY_RELEASE => Released,
                            _ => unreachable!(),
                        };

                        let device_id = mkdid(xev.sourceid);
                        let keycode = xev.detail;
                        if keycode < KEYCODE_OFFSET as u32 {
                            return;
                        }
                        let physical_key = keymap::raw_keycode_to_keycode(keycode);

                        callback(Event::DeviceEvent {
                            device_id,
                            event: DeviceEvent::Key(RawKeyEvent {
                                physical_key,
                                state,
                            }),
                        });
                    }

                    ffi::XCB_INPUT_HIERARCHY => {
                        let xev = unsafe {
                            &*(xev as *const _ as *const ffi::xcb_input_hierarchy_event_t)
                        };
                        let infos = unsafe {
                            slice::from_raw_parts(
                                wt.xconn.xinput.xcb_input_hierarchy_infos(xev),
                                xev.num_infos as _,
                            )
                        };
                        for info in infos {
                            if 0 != info.flags
                                & (ffi::XCB_INPUT_HIERARCHY_MASK_SLAVE_ADDED
                                    | ffi::XCB_INPUT_HIERARCHY_MASK_MASTER_ADDED)
                            {
                                Self::init_device(
                                    &self.target,
                                    &mut self.devices,
                                    &mut self.seats,
                                    info.deviceid,
                                );
                                callback(Event::DeviceEvent {
                                    device_id: mkdid(info.deviceid),
                                    event: DeviceEvent::Added,
                                });
                            } else if 0
                                != info.flags
                                    & (ffi::XCB_INPUT_HIERARCHY_MASK_SLAVE_REMOVED
                                        | ffi::XCB_INPUT_HIERARCHY_MASK_MASTER_REMOVED)
                            {
                                callback(Event::DeviceEvent {
                                    device_id: mkdid(info.deviceid),
                                    event: DeviceEvent::Removed,
                                });
                                self.devices.remove(&DeviceId(info.deviceid));
                                if info.type_ == ffi::XCB_INPUT_DEVICE_TYPE_MASTER_KEYBOARD as _ {
                                    self.seats.retain(|s| s.keyboard != info.deviceid);
                                }
                            }
                        }
                    }

                    _ => {}
                }
            }
            _ if response_type == wt.xconn.randr_first_event => {
                // In the future, it would be quite easy to emit monitor hotplug events.
                let prev_list = wt.xconn.invalidate_cached_monitor_list();
                if let Some(prev_list) = prev_list {
                    let new_list = wt.xconn.available_monitors();
                    for new_monitor in new_list {
                        prev_list
                            .iter()
                            .find(|prev_monitor| prev_monitor.name == new_monitor.name)
                            .map(|prev_monitor| {
                                if new_monitor.scale_factor != prev_monitor.scale_factor {
                                    for (window_id, window) in wt.windows.borrow().iter() {
                                        if let Some(window) = window.upgrade() {
                                            // Check if the window is on this monitor
                                            let monitor = window.current_monitor();
                                            if monitor.name == new_monitor.name {
                                                let (width, height) = window.inner_size_physical();
                                                let (new_width, new_height) = window
                                                    .adjust_for_dpi(
                                                        prev_monitor.scale_factor,
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
                                                let old_inner_size =
                                                    PhysicalSize::new(width, height);
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
                                                    let (new_width, new_height) =
                                                        new_inner_size.into();
                                                    window.set_inner_size_physical(
                                                        new_width, new_height,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            });
                    }
                }
            }
            _ if response_type == wt.xconn.xkb_first_event => {
                let xev = unsafe { &*(xev as *const _ as *const ffi::xcb_xkb_map_notify_event_t) };
                let seat = match find_seat(&mut self.seats, xev.device_id as _) {
                    Some(state) => state,
                    _ => return,
                };
                match xev.xkb_type {
                    ffi::XCB_XKB_MAP_NOTIFY | ffi::XCB_XKB_NEW_KEYBOARD_NOTIFY => {
                        unsafe {
                            seat.kb_state.init_with_x11_keymap(xev.device_id as _);
                        }
                        Self::modifiers_changed(seat, &mut callback);
                    }
                    ffi::XCB_XKB_STATE_NOTIFY => {
                        let xev = unsafe {
                            &*(xev as *const _ as *const ffi::xcb_xkb_state_notify_event_t)
                        };
                        Self::update_seat_kb(
                            seat,
                            xev.base_mods as u32,
                            xev.latched_mods as u32,
                            xev.locked_mods as u32,
                            xev.base_group as u32,
                            xev.latched_group as u32,
                            xev.locked_group as u32,
                            &mut callback,
                        );
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// Updates the window that has this seat's keyboard/pointer focus.
    ///
    /// This function must be called whenever an event occurs that could potentially imply a
    /// change of the focus. Some non-multi-seat-aware window managers treat all focus changes
    /// as changes of the core-seat focus. In these cases the X server will not send FocusIn/Out
    /// events for additional seats. By calling this function on every keyboard/pointer input,
    /// we update the focus as necessary.
    fn update_seat_focus<F>(
        seat: &mut Seat,
        component: SeatFocus,
        wt: &EventLoopWindowTarget<T>,
        mut focus: Option<ffi::xcb_window_t>,
        callback: &mut F,
    ) where
        F: FnMut(Event<'_, T>),
    {
        let old_focus_count = seat.kb_focus.is_some() as u8 + seat.ptr_focus.is_some() as u8;
        let seat_focus = match component {
            KbFocus => &mut seat.kb_focus,
            PtrFocus => &mut seat.ptr_focus,
        };
        if *seat_focus == focus {
            return;
        }
        if let Some(new) = focus {
            if !Self::window_exists(wt, new) {
                if seat_focus.is_none() {
                    return;
                }
                focus = None;
            }
        }
        if seat_focus.is_some() != focus.is_some() {
            let new_focus_count = if focus.is_some() {
                old_focus_count + 1
            } else {
                old_focus_count - 1
            };
            if (new_focus_count == 0) != (old_focus_count == 0) {
                let state_details = if new_focus_count == 0 {
                    0
                } else {
                    ffi::XCB_XKB_STATE_PART_MODIFIER_STATE as _
                };
                let details = ffi::xcb_xkb_select_events_details_t {
                    affect_state: ffi::XCB_XKB_STATE_PART_MODIFIER_STATE as _,
                    state_details,
                    ..Default::default()
                };
                let pending = wt.xconn.select_xkb_event_details(
                    seat.keyboard,
                    ffi::XCB_XKB_EVENT_TYPE_STATE_NOTIFY,
                    &details,
                );
                if let Err(e) = wt.xconn.check_pending1(pending) {
                    const MAX_ERRORS: usize = 5;
                    if seat.num_errors < MAX_ERRORS {
                        seat.num_errors += 1;
                        log::warn!("Could not change XKB selected events: {}", e);
                        if seat.num_errors == MAX_ERRORS {
                            log::warn!("Future warnings of this kind will be suppressed");
                        }
                    }
                }
            }
        }
        let device_id = mkdid(seat.keyboard);
        if let Some(focus) = *seat_focus {
            let event = match component {
                KbFocus => WindowEvent::Focused(false),
                PtrFocus => WindowEvent::CursorLeft { device_id },
            };
            callback(Event::WindowEvent {
                window_id: mkwid(focus),
                event,
            });
        }
        *seat_focus = focus;
        if let Some(focus) = *seat_focus {
            let event = match component {
                KbFocus => WindowEvent::Focused(true),
                PtrFocus => WindowEvent::CursorEntered { device_id },
            };
            callback(Event::WindowEvent {
                window_id: mkwid(focus),
                event,
            });
            callback(Event::WindowEvent {
                window_id: mkwid(focus),
                event: WindowEvent::ModifiersChanged(seat.current_modifiers),
            });
        }
    }

    fn update_seat_kb_xi<F>(
        seat: &mut Seat,
        mods: &ffi::xcb_input_modifier_info_t,
        group: &ffi::xcb_input_group_info_t,
        callback: &mut F,
    ) where
        F: FnMut(Event<'_, T>),
    {
        Self::update_seat_kb(
            seat,
            mods.base,
            mods.latched,
            mods.locked,
            group.base as u32,
            group.latched as u32,
            group.locked as u32,
            callback,
        );
    }

    fn update_seat_kb<F>(
        seat: &mut Seat,
        mods_depressed: u32,
        mods_latched: u32,
        mods_locked: u32,
        depressed_group: u32,
        latched_group: u32,
        locked_group: u32,
        callback: &mut F,
    ) where
        F: FnMut(Event<'_, T>),
    {
        seat.kb_state.update_state(
            mods_depressed,
            mods_latched,
            mods_locked,
            depressed_group,
            latched_group,
            locked_group,
        );
        Self::modifiers_changed(seat, callback);
    }

    fn modifiers_changed<F>(seat: &mut Seat, callback: &mut F)
    where
        F: FnMut(Event<'_, T>),
    {
        let new = seat.kb_state.mods_state();
        if seat.current_modifiers != new {
            seat.current_modifiers = new;
            let targets = [seat.kb_focus, seat.ptr_focus];
            let targets = if seat.kb_focus == seat.ptr_focus {
                &targets[..1]
            } else {
                &targets[..]
            };
            for target in targets {
                if let Some(target) = target {
                    callback(Event::WindowEvent {
                        window_id: mkwid(*target),
                        event: WindowEvent::ModifiersChanged(new),
                    });
                }
            }
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

fn find_seat(seats: &mut [Seat], kb: ffi::xcb_input_device_id_t) -> Option<&mut Seat> {
    seats.iter_mut().find(|s| s.keyboard == kb)
}

fn find_seat_by_pointer(
    seats: &mut [Seat],
    pointer: ffi::xcb_input_device_id_t,
) -> Option<&mut Seat> {
    seats.iter_mut().find(|s| s.pointer == pointer)
}

fn mask_to_axis_ids(mask: &[u32]) -> Vec<u16> {
    let mut axis_ids = vec![];
    for i in 0..mask.len() {
        for j in 0..32 {
            if (mask[i] >> j) & 1 == 1 {
                axis_ids.push((i * 32 + j) as u16);
            }
        }
    }
    axis_ids
}
