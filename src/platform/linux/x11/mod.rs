#![cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd"))]

pub mod ffi;
mod events;
mod monitor;
mod window;
mod xdisplay;
mod dnd;
mod ime;
pub mod util;

pub use self::monitor::MonitorId;
pub use self::window::UnownedWindow;
pub use self::xdisplay::{XConnection, XNotSupported, XError};

use std::{mem, ptr, slice};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::CStr;
use std::ops::Deref;
use std::os::raw::*;
use libc::{select, fd_set, FD_SET, FD_ZERO, FD_ISSET, EINTR, EINVAL, ENOMEM, EBADF};
#[cfg(target_os = "linux")]
use libc::__errno_location;
#[cfg(target_os = "freebsd")]
use libc::__error as __errno_location;
#[cfg(any(target_os = "netbsd", target_os = "openbsd"))]
use libc::__errno as __errno_location;
use std::sync::{Arc, mpsc, Weak};
use std::sync::atomic::{self, AtomicBool};

use libc::{self, setlocale, LC_CTYPE};

use {
    ControlFlow,
    CreationError,
    DeviceEvent,
    Event,
    EventsLoopClosed,
    KeyboardInput,
    LogicalPosition,
    LogicalSize,
    WindowAttributes,
    WindowEvent,
};
use events::ModifiersState;
use platform::PlatformSpecificWindowBuilderAttributes;
use self::dnd::{Dnd, DndState};
use self::ime::{ImeReceiver, ImeSender, ImeCreationError, Ime};

pub struct EventsLoop {
    xconn: Arc<XConnection>,
    wm_delete_window: ffi::Atom,
    dnd: Dnd,
    ime_receiver: ImeReceiver,
    ime_sender: ImeSender,
    ime: RefCell<Ime>,
    randr_event_offset: c_int,
    windows: RefCell<HashMap<WindowId, Weak<UnownedWindow>>>,
    devices: RefCell<HashMap<DeviceId, Device>>,
    xi2ext: XExtension,
    pending_wakeup: Arc<AtomicBool>,
    root: ffi::Window,
    // A dummy, `InputOnly` window that we can use to receive wakeup events and interrupt blocking
    // `XNextEvent` calls.
    wakeup_dummy_window: ffi::Window,
}

#[derive(Clone)]
pub struct EventsLoopProxy {
    pending_wakeup: Weak<AtomicBool>,
    xconn: Weak<XConnection>,
    wakeup_dummy_window: ffi::Window,
}

impl EventsLoop {
    pub fn new(xconn: Arc<XConnection>) -> EventsLoop {
        let root = unsafe { (xconn.xlib.XDefaultRootWindow)(xconn.display) };

        let wm_delete_window = unsafe { xconn.get_atom_unchecked(b"WM_DELETE_WINDOW\0") };

        let dnd = Dnd::new(Arc::clone(&xconn))
            .expect("Failed to call XInternAtoms when initializing drag and drop");

        let (ime_sender, ime_receiver) = mpsc::channel();
        // Input methods will open successfully without setting the locale, but it won't be
        // possible to actually commit pre-edit sequences.
        unsafe { setlocale(LC_CTYPE, b"\0".as_ptr() as *const _); }
        let ime = RefCell::new({
            let result = Ime::new(Arc::clone(&xconn));
            if let Err(ImeCreationError::OpenFailure(ref state)) = result {
                panic!(format!("Failed to open input method: {:#?}", state));
            }
            result.expect("Failed to set input method destruction callback")
        });

        let randr_event_offset = xconn.select_xrandr_input(root)
            .expect("Failed to query XRandR extension");

        let xi2ext = unsafe {
            let mut result = XExtension {
                opcode: mem::uninitialized(),
                first_event_id: mem::uninitialized(),
                first_error_id: mem::uninitialized(),
            };
            let res = (xconn.xlib.XQueryExtension)(
                xconn.display,
                b"XInputExtension\0".as_ptr() as *const c_char,
                &mut result.opcode as *mut c_int,
                &mut result.first_event_id as *mut c_int,
                &mut result.first_error_id as *mut c_int);
            if res == ffi::False {
                panic!("X server missing XInput extension");
            }
            result
        };

        unsafe {
            let mut xinput_major_ver = ffi::XI_2_Major;
            let mut xinput_minor_ver = ffi::XI_2_Minor;
            if (xconn.xinput2.XIQueryVersion)(
                xconn.display,
                &mut xinput_major_ver,
                &mut xinput_minor_ver,
            ) != ffi::Success as libc::c_int {
                panic!(
                    "X server has XInput extension {}.{} but does not support XInput2",
                    xinput_major_ver,
                    xinput_minor_ver,
                );
            }
        }

        xconn.update_cached_wm_info(root);

        let wakeup_dummy_window = unsafe {
            let (x, y, w, h) = (10, 10, 10, 10);
            let (border_w, border_px, background_px) = (0, 0, 0);
            (xconn.xlib.XCreateSimpleWindow)(
                xconn.display,
                root,
                x,
                y,
                w,
                h,
                border_w,
                border_px,
                background_px,
            )
        };

        let result = EventsLoop {
            xconn,
            wm_delete_window,
            dnd,
            ime_receiver,
            ime_sender,
            ime,
            randr_event_offset,
            windows: Default::default(),
            devices: Default::default(),
            xi2ext,
            pending_wakeup: Default::default(),
            root,
            wakeup_dummy_window,
        };

        // Register for device hotplug events
        // (The request buffer is flushed during `init_device`)
        result.xconn.select_xinput_events(
            root,
            ffi::XIAllDevices,
            ffi::XI_HierarchyChangedMask,
        ).queue();

        result.init_device(ffi::XIAllDevices);

        result
    }

    /// Returns the `XConnection` of this events loop.
    #[inline]
    pub fn x_connection(&self) -> &Arc<XConnection> {
        &self.xconn
    }

    pub fn create_proxy(&self) -> EventsLoopProxy {
        EventsLoopProxy {
            pending_wakeup: Arc::downgrade(&self.pending_wakeup),
            xconn: Arc::downgrade(&self.xconn),
            wakeup_dummy_window: self.wakeup_dummy_window,
        }
    }

    unsafe fn poll_one_event(&mut self, event_ptr : *mut ffi::XEvent) -> bool {
        // This function is used to poll and remove a single event
        // from the Xlib event queue in a non-blocking, atomic way.
        // XCheckIfEvent is non-blocking and removes events from queue.
        // XNextEvent can't be used because it blocks while holding the
        // global Xlib mutex.
        // XPeekEvent does not remove events from the queue.
        unsafe extern "C" fn predicate(
            _display: *mut ffi::Display,
            _event: *mut ffi::XEvent,
            _arg : *mut c_char)  -> c_int {
            // This predicate always returns "true" (1) to accept all events
            1
        }

        let result = (self.xconn.xlib.XCheckIfEvent)(
            self.xconn.display,
            event_ptr,
            Some(predicate),
            std::ptr::null_mut());

        result != 0
    }

    unsafe fn wait_for_input(&mut self) {
        // XNextEvent can not be used in multi-threaded applications
        // because it is blocking for input while holding the global
        // Xlib mutex.
        // To work around this issue, first flush the X11 display, then
        // use select(2) to wait for input to arrive
        loop {
            // First use XFlush to flush any buffered x11 requests
            (self.xconn.xlib.XFlush)(self.xconn.display);

            // Then use select(2) to wait for input data
            let mut fds : fd_set = mem::uninitialized();
            FD_ZERO(&mut fds);
            FD_SET(self.xconn.x11_fd, &mut fds);
            let err = select(
                self.xconn.x11_fd + 1,
                &mut fds, // read fds
                std::ptr::null_mut(), // write fds
                std::ptr::null_mut(), // except fds (could be used to detect errors)
                std::ptr::null_mut()); // timeout

            if err < 0 {
                let errno_ptr = __errno_location();
                let errno = *errno_ptr;

                if errno == EINTR {
                    // try again if errno is EINTR
                    continue;
                }

                assert!(errno == EBADF || errno == EINVAL || errno == ENOMEM);
                panic!("select(2) returned fatal error condition");
            }

            if FD_ISSET(self.xconn.x11_fd, &mut fds) {
                break;
            }
        }
    }

    pub fn poll_events<F>(&mut self, mut callback: F)
        where F: FnMut(Event)
    {
        let mut xev = unsafe { mem::uninitialized() };
        loop {
            // Get next event
            unsafe {
                if !self.poll_one_event(&mut xev) {
                    break;
                }
            }
            self.process_event(&mut xev, &mut callback);
        }
    }

    pub fn run_forever<F>(&mut self, mut callback: F)
        where F: FnMut(Event) -> ControlFlow
    {
        let mut xev = unsafe { mem::uninitialized() };

        loop {
            unsafe {
                while !self.poll_one_event(&mut xev) {
                    // block until input is available
                    self.wait_for_input();
                }
            };

            let mut control_flow = ControlFlow::Continue;

            // Track whether or not `Break` was returned when processing the event.
            {
                let mut cb = |event| {
                    if let ControlFlow::Break = callback(event) {
                        control_flow = ControlFlow::Break;
                    }
                };

                self.process_event(&mut xev, &mut cb);
            }

            if let ControlFlow::Break = control_flow {
                break;
            }
        }
    }

    fn process_event<F>(&mut self, xev: &mut ffi::XEvent, mut callback: F)
        where F: FnMut(Event)
    {
        // XFilterEvent tells us when an event has been discarded by the input method.
        // Specifically, this involves all of the KeyPress events in compose/pre-edit sequences,
        // along with an extra copy of the KeyRelease events. This also prevents backspace and
        // arrow keys from being detected twice.
        if ffi::True == unsafe { (self.xconn.xlib.XFilterEvent)(
            xev,
            { let xev: &ffi::XAnyEvent = xev.as_ref(); xev.window }
        ) } {
            return;
        }

        let event_type = xev.get_type();
        match event_type {
            ffi::MappingNotify => {
                unsafe { (self.xconn.xlib.XRefreshKeyboardMapping)(xev.as_mut()); }
                self.xconn.check_errors().expect("Failed to call XRefreshKeyboardMapping");
            }

            ffi::ClientMessage => {
                let client_msg: &ffi::XClientMessageEvent = xev.as_ref();

                let window = client_msg.window;
                let window_id = mkwid(window);

                if client_msg.data.get_long(0) as ffi::Atom == self.wm_delete_window {
                    callback(Event::WindowEvent { window_id, event: WindowEvent::CloseRequested });
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
                            client_msg.data.get_long(4) as c_ulong
                        ];
                        self.dnd.type_list = Some(type_list);
                    } else if let Ok(more_types) = unsafe { self.dnd.get_type_list(source_window) } {
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
                            self.dnd.send_status(window, source_window, DndState::Accepted)
                                .expect("Failed to send `XdndStatus` message.");
                        }
                    } else {
                        unsafe {
                            self.dnd.send_status(window, source_window, DndState::Rejected)
                                .expect("Failed to send `XdndStatus` message.");
                        }
                        self.dnd.reset();
                    }
                } else if client_msg.message_type == self.dnd.atoms.drop {
                    let (source_window, state) = if let Some(source_window) = self.dnd.source_window {
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
                        self.dnd.send_finished(window, source_window, state)
                            .expect("Failed to send `XdndFinished` message.");
                    }
                    self.dnd.reset();
                } else if client_msg.message_type == self.dnd.atoms.leave {
                    self.dnd.reset();
                    callback(Event::WindowEvent {
                        window_id,
                        event: WindowEvent::HoveredFileCancelled,
                    });
                } else if self.pending_wakeup.load(atomic::Ordering::Relaxed) {
                    self.pending_wakeup.store(false, atomic::Ordering::Relaxed);
                    callback(Event::Awakened);
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

                    let mut monitor = window.get_current_monitor(); // This must be done *before* locking!
                    let mut shared_state_lock = window.shared_state.lock();

                    let (mut resized, moved) = {
                        let resized = util::maybe_change(&mut shared_state_lock.size, new_inner_size);
                        let moved = if is_synthetic {
                            util::maybe_change(&mut shared_state_lock.inner_position, new_inner_position)
                        } else {
                            // Detect when frame extents change.
                            // Since this isn't synthetic, as per the notes above, this position is relative to the
                            // parent window.
                            let rel_parent = new_inner_position;
                            if util::maybe_change(&mut shared_state_lock.inner_position_rel_parent, rel_parent) {
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
                        let frame_extents = shared_state_lock.frame_extents
                            .as_ref()
                            .cloned()
                            .unwrap_or_else(|| {
                                let frame_extents = self.xconn.get_frame_extents_heuristic(xwindow, self.root);
                                shared_state_lock.frame_extents = Some(frame_extents.clone());
                                frame_extents
                            });
                        let outer = frame_extents.inner_pos_to_outer(new_inner_position.0, new_inner_position.1);
                        shared_state_lock.position = Some(outer);
                        if moved {
                            let logical_position = LogicalPosition::from_physical(outer, monitor.hidpi_factor);
                            events.moved = Some(WindowEvent::Moved(logical_position));
                        }
                        outer
                    } else {
                        shared_state_lock.position.unwrap()
                    };

                    if is_synthetic {
                        // If we don't use the existing adjusted value when available, then the user can screw up the
                        // resizing by dragging across monitors *without* dropping the window.
                        let (width, height) = shared_state_lock.dpi_adjusted
                            .unwrap_or_else(|| (xev.width as f64, xev.height as f64));
                        let last_hidpi_factor = shared_state_lock.guessed_dpi
                            .take()
                            .unwrap_or_else(|| {
                                shared_state_lock.last_monitor
                                    .as_ref()
                                    .map(|last_monitor| last_monitor.hidpi_factor)
                                    .unwrap_or(1.0)
                            });
                        let new_hidpi_factor = {
                            let window_rect = util::AaRect::new(new_outer_position, new_inner_size);
                            monitor = self.xconn.get_monitor_for_window(Some(window_rect));
                            let new_hidpi_factor = monitor.hidpi_factor;
                            shared_state_lock.last_monitor = Some(monitor.clone());
                            new_hidpi_factor
                        };
                        if last_hidpi_factor != new_hidpi_factor {
                            events.dpi_changed = Some(WindowEvent::HiDpiFactorChanged(new_hidpi_factor));
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
                        let rounded_size = (adjusted_size.0.round() as u32, adjusted_size.1.round() as u32);
                        if new_inner_size == rounded_size || !util::wm_name_is_one_of(&["Xfwm4"]) {
                            // When this finally happens, the event will not be synthetic.
                            shared_state_lock.dpi_adjusted = None;
                        } else {
                            unsafe {
                                (self.xconn.xlib.XResizeWindow)(
                                    self.xconn.display,
                                    xwindow,
                                    rounded_size.0 as c_uint,
                                    rounded_size.1 as c_uint,
                                );
                            }
                        }
                    }

                    if resized {
                        let logical_size = LogicalSize::from_physical(new_inner_size, monitor.hidpi_factor);
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
                self.xconn.update_cached_wm_info(self.root);

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
                self.windows.borrow_mut().remove(&WindowId(window));

                // Since all XIM stuff needs to happen from the same thread, we destroy the input
                // context here instead of when dropping the window.
                self.ime
                    .borrow_mut()
                    .remove_context(window)
                    .expect("Failed to destroy input context");

                callback(Event::WindowEvent { window_id, event: WindowEvent::Destroyed });
            }

            ffi::Expose => {
                let xev: &ffi::XExposeEvent = xev.as_ref();

                let window = xev.window;
                let window_id = mkwid(window);

                callback(Event::WindowEvent { window_id, event: WindowEvent::Refresh });
            }

            ffi::KeyPress | ffi::KeyRelease => {
                use events::ElementState::{Pressed, Released};

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

                // When a compose sequence or IME pre-edit is finished, it ends in a KeyPress with
                // a keycode of 0.
                if xkev.keycode != 0 {
                    let modifiers = ModifiersState {
                        alt: xkev.state & ffi::Mod1Mask != 0,
                        shift: xkev.state & ffi::ShiftMask != 0,
                        ctrl: xkev.state & ffi::ControlMask != 0,
                        logo: xkev.state & ffi::Mod4Mask != 0,
                    };

                    let keysym = unsafe {
                        let mut keysym = 0;
                        (self.xconn.xlib.XLookupString)(
                            xkev,
                            ptr::null_mut(),
                            0,
                            &mut keysym,
                            ptr::null_mut(),
                        );
                        self.xconn.check_errors().expect("Failed to lookup keysym");
                        keysym
                    };
                    let virtual_keycode = events::keysym_to_element(keysym as c_uint);

                    callback(Event::WindowEvent {
                        window_id,
                        event: WindowEvent::KeyboardInput {
                            device_id,
                            input: KeyboardInput {
                                state,
                                scancode: xkev.keycode - 8,
                                virtual_keycode,
                                modifiers,
                            },
                        }
                    });
                }

                if state == Pressed {
                    let written = if let Some(ic) = self.ime.borrow().get_context(window) {
                        self.xconn.lookup_utf8(ic, xkev)
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
                let guard = if let Some(e) = GenericEventCookie::from_event(&self.xconn, *xev) { e } else { return };
                let xev = &guard.cookie;
                if self.xi2ext.opcode != xev.extension {
                    return;
                }

                use events::WindowEvent::{Focused, CursorEntered, MouseInput, CursorLeft, CursorMoved, MouseWheel, AxisMotion};
                use events::ElementState::{Pressed, Released};
                use events::MouseButton::{Left, Right, Middle, Other};
                use events::MouseScrollDelta::LineDelta;
                use events::{Touch, TouchPhase};

                match xev.evtype {
                    ffi::XI_ButtonPress | ffi::XI_ButtonRelease => {
                        let xev: &ffi::XIDeviceEvent = unsafe { &*(xev.data as *const _) };
                        let window_id = mkwid(xev.event);
                        let device_id = mkdid(xev.deviceid);
                        if (xev.flags & ffi::XIPointerEmulated) != 0 {
                            // Deliver multi-touch events instead of emulated mouse events.
                            let return_now = self
                                .with_window(xev.event, |window| window.multitouch)
                                .unwrap_or(true);
                            if return_now { return; }
                        }

                        let modifiers = ModifiersState::from(xev.mods);

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
                            4 | 5 | 6 | 7 => if xev.flags & ffi::XIPointerEmulated == 0 {
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
                            },

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

                        let modifiers = ModifiersState::from(xev.mods);

                        let cursor_moved = self.with_window(xev.event, |window| {
                            let mut shared_state_lock = window.shared_state.lock();
                            util::maybe_change(&mut shared_state_lock.cursor_pos, new_cursor_pos)
                        });
                        if cursor_moved == Some(true) {
                            let dpi_factor = self.with_window(xev.event, |window| {
                                window.get_hidpi_factor()
                            });
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
                            let mask = unsafe { slice::from_raw_parts(xev.valuators.mask, xev.valuators.mask_len as usize) };
                            let mut devices = self.devices.borrow_mut();
                            let physical_device = match devices.get_mut(&DeviceId(xev.sourceid)) {
                                Some(device) => device,
                                None => return,
                            };

                            let mut value = xev.valuators.values;
                            for i in 0..xev.valuators.mask_len*8 {
                                if ffi::XIMaskIsSet(mask, i) {
                                    let x = unsafe { *value };
                                    if let Some(&mut (_, ref mut info)) = physical_device.scroll_axes.iter_mut().find(|&&mut (axis, _)| axis == i) {
                                        let delta = (x - info.position) / info.increment;
                                        info.position = x;
                                        events.push(Event::WindowEvent {
                                            window_id,
                                            event: MouseWheel {
                                                device_id,
                                                delta: match info.orientation {
                                                    ScrollOrientation::Horizontal => LineDelta(delta as f32, 0.0),
                                                    // X11 vertical scroll coordinates are opposite to winit's
                                                    ScrollOrientation::Vertical => LineDelta(0.0, -delta as f32),
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

                        if let Some(all_info) = DeviceInfo::get(&self.xconn, ffi::XIAllDevices) {
                            let mut devices = self.devices.borrow_mut();
                            for device_info in all_info.iter() {
                                if device_info.deviceid == xev.sourceid
                                // This is needed for resetting to work correctly on i3, and
                                // presumably some other WMs. On those, `XI_Enter` doesn't include
                                // the physical device ID, so both `sourceid` and `deviceid` are
                                // the virtual device.
                                || device_info.attachment == xev.sourceid {
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

                        if let Some(dpi_factor) = self.with_window(xev.event, |window| {
                            window.get_hidpi_factor()
                        }) {
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
                            let modifiers = self.xconn
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
                                event: CursorLeft { device_id: mkdid(xev.deviceid) },
                            });
                        }
                    }
                    ffi::XI_FocusIn => {
                        let xev: &ffi::XIFocusInEvent = unsafe { &*(xev.data as *const _) };

                        let dpi_factor = match self.with_window(xev.event, |window| {
                            window.get_hidpi_factor()
                        }) {
                            Some(dpi_factor) => dpi_factor,
                            None => return,
                        };
                        let window_id = mkwid(xev.event);

                        self.ime
                            .borrow_mut()
                            .focus(xev.event)
                            .expect("Failed to focus input context");

                        callback(Event::WindowEvent { window_id, event: Focused(true) });

                        // The deviceid for this event is for a keyboard instead of a pointer,
                        // so we have to do a little extra work.
                        let pointer_id = self.devices
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
                                modifiers: ModifiersState::from(xev.mods),
                            }
                        });
                    }
                    ffi::XI_FocusOut => {
                        let xev: &ffi::XIFocusOutEvent = unsafe { &*(xev.data as *const _) };
                        if !self.window_exists(xev.event) { return; }
                        self.ime
                            .borrow_mut()
                            .unfocus(xev.event)
                            .expect("Failed to unfocus input context");
                        callback(Event::WindowEvent {
                            window_id: mkwid(xev.event),
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
                            _ => unreachable!()
                        };
                         let dpi_factor = self.with_window(xev.event, |window| {
                            window.get_hidpi_factor()
                        });
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
                                    id: xev.detail as u64,
                                }),
                            })
                        }
                    }

                    ffi::XI_RawButtonPress | ffi::XI_RawButtonRelease => {
                        let xev: &ffi::XIRawEvent = unsafe { &*(xev.data as *const _) };
                        if xev.flags & ffi::XIPointerEmulated == 0 {
                            callback(Event::DeviceEvent { device_id: mkdid(xev.deviceid), event: DeviceEvent::Button {
                                button: xev.detail as u32,
                                state: match xev.evtype {
                                    ffi::XI_RawButtonPress => Pressed,
                                    ffi::XI_RawButtonRelease => Released,
                                    _ => unreachable!(),
                                },
                            }});
                        }
                    }

                    ffi::XI_RawMotion => {
                        let xev: &ffi::XIRawEvent = unsafe { &*(xev.data as *const _) };
                        let did = mkdid(xev.deviceid);

                        let mask = unsafe { slice::from_raw_parts(xev.valuators.mask, xev.valuators.mask_len as usize) };
                        let mut value = xev.raw_values;
                        let mut mouse_delta = (0.0, 0.0);
                        let mut scroll_delta = (0.0, 0.0);
                        for i in 0..xev.valuators.mask_len*8 {
                            if ffi::XIMaskIsSet(mask, i) {
                                let x = unsafe { *value };
                                // We assume that every XInput2 device with analog axes is a pointing device emitting
                                // relative coordinates.
                                match i {
                                    0 => mouse_delta.0 = x,
                                    1 => mouse_delta.1 = x,
                                    2 => scroll_delta.0 = x as f32,
                                    3 => scroll_delta.1 = x as f32,
                                    _ => {},
                                }
                                callback(Event::DeviceEvent { device_id: did, event: DeviceEvent::Motion {
                                    axis: i as u32,
                                    value: x,
                                }});
                                value = unsafe { value.offset(1) };
                            }
                        }
                        if mouse_delta != (0.0, 0.0) {
                            callback(Event::DeviceEvent { device_id: did, event: DeviceEvent::MouseMotion {
                                delta: mouse_delta,
                            }});
                        }
                        if scroll_delta != (0.0, 0.0) {
                            callback(Event::DeviceEvent { device_id: did, event: DeviceEvent::MouseWheel {
                                delta: LineDelta(scroll_delta.0, scroll_delta.1),
                            }});
                        }
                    }

                    ffi::XI_RawKeyPress | ffi::XI_RawKeyRelease => {
                        let xev: &ffi::XIRawEvent = unsafe { &*(xev.data as *const _) };

                        let state = match xev.evtype {
                            ffi::XI_RawKeyPress => Pressed,
                            ffi::XI_RawKeyRelease => Released,
                            _ => unreachable!(),
                        };

                        let device_id = xev.sourceid;
                        let keycode = xev.detail;
                        if keycode < 8 { return; }
                        let scancode = (keycode - 8) as u32;

                        let keysym = unsafe {
                            (self.xconn.xlib.XKeycodeToKeysym)(
                                self.xconn.display,
                                xev.detail as ffi::KeyCode,
                                0,
                            )
                        };
                        self.xconn.check_errors().expect("Failed to lookup raw keysym");

                        let virtual_keycode = events::keysym_to_element(keysym as c_uint);

                        callback(Event::DeviceEvent {
                            device_id: mkdid(device_id),
                            event: DeviceEvent::Key(KeyboardInput {
                                scancode,
                                virtual_keycode,
                                state,
                                // So, in an ideal world we can use libxkbcommon to get modifiers.
                                // However, libxkbcommon-x11 isn't as commonly installed as one
                                // would hope. We can still use the Xkb extension to get
                                // comprehensive keyboard state updates, but interpreting that
                                // info manually is going to be involved.
                                modifiers: ModifiersState::default(),
                            }),
                        });
                    }

                    ffi::XI_HierarchyChanged => {
                        let xev: &ffi::XIHierarchyEvent = unsafe { &*(xev.data as *const _) };
                        for info in unsafe { slice::from_raw_parts(xev.info, xev.num_info as usize) } {
                            if 0 != info.flags & (ffi::XISlaveAdded | ffi::XIMasterAdded) {
                                self.init_device(info.deviceid);
                                callback(Event::DeviceEvent { device_id: mkdid(info.deviceid), event: DeviceEvent::Added });
                            } else if 0 != info.flags & (ffi::XISlaveRemoved | ffi::XIMasterRemoved) {
                                callback(Event::DeviceEvent { device_id: mkdid(info.deviceid), event: DeviceEvent::Removed });
                                let mut devices = self.devices.borrow_mut();
                                devices.remove(&DeviceId(info.deviceid));
                            }
                        }
                    }

                    _ => {}
                }
            },
            _ => {
                if event_type == self.randr_event_offset {
                    // In the future, it would be quite easy to emit monitor hotplug events.
                    let prev_list = monitor::invalidate_cached_monitor_list();
                    if let Some(prev_list) = prev_list {
                        let new_list = self.xconn.get_available_monitors();
                        for new_monitor in new_list {
                            prev_list
                                .iter()
                                .find(|prev_monitor| prev_monitor.name == new_monitor.name)
                                .map(|prev_monitor| {
                                    if new_monitor.hidpi_factor != prev_monitor.hidpi_factor {
                                        for (window_id, window) in self.windows.borrow().iter() {
                                            if let Some(window) = window.upgrade() {
                                                // Check if the window is on this monitor
                                                let monitor = window.get_current_monitor();
                                                if monitor.name == new_monitor.name {
                                                    callback(Event::WindowEvent {
                                                        window_id: mkwid(window_id.0),
                                                        event: WindowEvent::HiDpiFactorChanged(
                                                            new_monitor.hidpi_factor
                                                        ),
                                                    });
                                                    let (width, height) = match window.get_inner_size_physical() {
                                                        Some(result) => result,
                                                        None => continue,
                                                    };
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
            },
        }

        match self.ime_receiver.try_recv() {
            Ok((window_id, x, y)) => {
                self.ime.borrow_mut().send_xim_spot(window_id, x, y);
            },
            Err(_) => (),
        }
    }

    fn init_device(&self, device: c_int) {
        let mut devices = self.devices.borrow_mut();
        if let Some(info) = DeviceInfo::get(&self.xconn, device) {
            for info in info.iter() {
                devices.insert(DeviceId(info.deviceid), Device::new(&self, info));
            }
        }
    }

    fn with_window<F, T>(&self, window_id: ffi::Window, callback: F) -> Option<T>
        where F: Fn(&UnownedWindow) -> T
    {
        let mut deleted = false;
        let window_id = WindowId(window_id);
        let result = self.windows
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
            self.windows.borrow_mut().remove(&window_id);
        }
        result
    }

    fn window_exists(&self, window_id: ffi::Window) -> bool {
        self.with_window(window_id, |_| ()).is_some()
    }
}

impl EventsLoopProxy {
    pub fn wakeup(&self) -> Result<(), EventsLoopClosed> {
        // Update the `EventsLoop`'s `pending_wakeup` flag.
        let display = match (self.pending_wakeup.upgrade(), self.xconn.upgrade()) {
            (Some(wakeup), Some(display)) => {
                wakeup.store(true, atomic::Ordering::Relaxed);
                display
            },
            _ => return Err(EventsLoopClosed),
        };

        // Push an event on the X event queue so that methods run_forever will advance.
        //
        // NOTE: This design is taken from the old `WindowProxy::wakeup` implementation. It
        // assumes that X11 is thread safe. Is this true?
        // (WARNING: it's probably not true)
        display.send_client_msg(
            self.wakeup_dummy_window,
            self.wakeup_dummy_window,
            0,
            None,
            [0, 0, 0, 0, 0],
        ).flush().expect("Failed to call XSendEvent after wakeup");

        Ok(())
    }
}

struct DeviceInfo<'a> {
    xconn: &'a XConnection,
    info: *const ffi::XIDeviceInfo,
    count: usize,
}

impl<'a> DeviceInfo<'a> {
    fn get(xconn: &'a XConnection, device: c_int) -> Option<Self> {
        unsafe {
            let mut count = mem::uninitialized();
            let info = (xconn.xinput2.XIQueryDevice)(xconn.display, device, &mut count);
            xconn.check_errors()
                .ok()
                .and_then(|_| {
                    if info.is_null() || count == 0 {
                        None
                    } else {
                        Some(DeviceInfo {
                            xconn,
                            info,
                            count: count as usize,
                        })
                    }
                })
        }
    }
}

impl<'a> Drop for DeviceInfo<'a> {
    fn drop(&mut self) {
        assert!(!self.info.is_null());
        unsafe { (self.xconn.xinput2.XIFreeDeviceInfo)(self.info as *mut _) };
    }
}

impl<'a> Deref for DeviceInfo<'a> {
    type Target = [ffi::XIDeviceInfo];
    fn deref(&self) -> &Self::Target {
        unsafe { slice::from_raw_parts(self.info, self.count) }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(ffi::Window);

impl WindowId {
    pub unsafe fn dummy() -> Self {
        WindowId(0)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(c_int);

impl DeviceId {
    pub unsafe fn dummy() -> Self {
        DeviceId(0)
    }
}

pub struct Window(Arc<UnownedWindow>);

impl Deref for Window {
    type Target = UnownedWindow;
    #[inline]
    fn deref(&self) -> &UnownedWindow {
        &*self.0
    }
}

impl Window {
    pub fn new(
        event_loop: &EventsLoop,
        attribs: WindowAttributes,
        pl_attribs: PlatformSpecificWindowBuilderAttributes
    ) -> Result<Self, CreationError> {
        let window = Arc::new(UnownedWindow::new(&event_loop, attribs, pl_attribs)?);
        event_loop.windows
            .borrow_mut()
            .insert(window.id(), Arc::downgrade(&window));
        Ok(Window(window))
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        let window = self.deref();
        let xconn = &window.xconn;
        unsafe {
            (xconn.xlib.XDestroyWindow)(xconn.display, window.id().0);
            // If the window was somehow already destroyed, we'll get a `BadWindow` error, which we don't care about.
            let _ = xconn.check_errors();
        }
    }
}

/// XEvents of type GenericEvent store their actual data in an XGenericEventCookie data structure. This is a wrapper to
/// extract the cookie from a GenericEvent XEvent and release the cookie data once it has been processed
struct GenericEventCookie<'a> {
    xconn: &'a XConnection,
    cookie: ffi::XGenericEventCookie
}

impl<'a> GenericEventCookie<'a> {
    fn from_event<'b>(xconn: &'b XConnection, event: ffi::XEvent) -> Option<GenericEventCookie<'b>> {
        unsafe {
            let mut cookie: ffi::XGenericEventCookie = From::from(event);
            if (xconn.xlib.XGetEventData)(xconn.display, &mut cookie) == ffi::True {
                Some(GenericEventCookie { xconn, cookie })
            } else {
                None
            }
        }
    }
}

impl<'a> Drop for GenericEventCookie<'a> {
    fn drop(&mut self) {
        unsafe {
            (self.xconn.xlib.XFreeEventData)(self.xconn.display, &mut self.cookie);
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct XExtension {
    opcode: c_int,
    first_event_id: c_int,
    first_error_id: c_int,
}

fn mkwid(w: ffi::Window) -> ::WindowId { ::WindowId(::platform::WindowId::X(WindowId(w))) }
fn mkdid(w: c_int) -> ::DeviceId { ::DeviceId(::platform::DeviceId::X(DeviceId(w))) }

#[derive(Debug)]
struct Device {
    name: String,
    scroll_axes: Vec<(i32, ScrollAxis)>,
    // For master devices, this is the paired device (pointer <-> keyboard).
    // For slave devices, this is the master.
    attachment: c_int,
}

#[derive(Debug, Copy, Clone)]
struct ScrollAxis {
    increment: f64,
    orientation: ScrollOrientation,
    position: f64,
}

#[derive(Debug, Copy, Clone)]
enum ScrollOrientation {
    Vertical,
    Horizontal,
}

impl Device {
    fn new(el: &EventsLoop, info: &ffi::XIDeviceInfo) -> Self {
        let name = unsafe { CStr::from_ptr(info.name).to_string_lossy() };
        let mut scroll_axes = Vec::new();

        if Device::physical_device(info) {
            // Register for global raw events
            let mask = ffi::XI_RawMotionMask
                | ffi::XI_RawButtonPressMask
                | ffi::XI_RawButtonReleaseMask
                | ffi::XI_RawKeyPressMask
                | ffi::XI_RawKeyReleaseMask;
            // The request buffer is flushed when we poll for events
            el.xconn.select_xinput_events(el.root, info.deviceid, mask).queue();

            // Identify scroll axes
            for class_ptr in Device::classes(info) {
                let class = unsafe { &**class_ptr };
                match class._type {
                    ffi::XIScrollClass => {
                        let info = unsafe { mem::transmute::<&ffi::XIAnyClassInfo, &ffi::XIScrollClassInfo>(class) };
                        scroll_axes.push((info.number, ScrollAxis {
                            increment: info.increment,
                            orientation: match info.scroll_type {
                                ffi::XIScrollTypeHorizontal => ScrollOrientation::Horizontal,
                                ffi::XIScrollTypeVertical => ScrollOrientation::Vertical,
                                _ => { unreachable!() }
                            },
                            position: 0.0,
                        }));
                    }
                    _ => {}
                }
            }
        }

        let mut device = Device {
            name: name.into_owned(),
            scroll_axes: scroll_axes,
            attachment: info.attachment,
        };
        device.reset_scroll_position(info);
        device
    }

    fn reset_scroll_position(&mut self, info: &ffi::XIDeviceInfo) {
        if Device::physical_device(info) {
            for class_ptr in Device::classes(info) {
                let class = unsafe { &**class_ptr };
                match class._type {
                    ffi::XIValuatorClass => {
                        let info = unsafe { mem::transmute::<&ffi::XIAnyClassInfo, &ffi::XIValuatorClassInfo>(class) };
                        if let Some(&mut (_, ref mut axis)) = self.scroll_axes.iter_mut().find(|&&mut (axis, _)| axis == info.number) {
                            axis.position = info.value;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    #[inline]
    fn physical_device(info: &ffi::XIDeviceInfo) -> bool {
        info._use == ffi::XISlaveKeyboard || info._use == ffi::XISlavePointer || info._use == ffi::XIFloatingSlave
    }

    #[inline]
    fn classes(info: &ffi::XIDeviceInfo) -> &[*const ffi::XIAnyClassInfo] {
        unsafe { slice::from_raw_parts(info.classes as *const *const ffi::XIAnyClassInfo, info.num_classes as usize) }
    }
}
