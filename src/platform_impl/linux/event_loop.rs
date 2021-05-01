// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{
    cell::RefCell,
    collections::{HashSet, VecDeque},
    error::Error,
    process,
    rc::Rc,
    sync::mpsc::{channel, Receiver, SendError, Sender},
};

use gdk::{Cursor, CursorType, WindowExt, WindowState};
use gio::{prelude::*, Cancellable};
use glib::{source::idle_add_local, Continue, MainContext};
use gtk::{prelude::*, ApplicationWindow, Inhibit};

use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{
        DeviceId as RootDeviceId, ElementState, Event, ModifiersState, MouseButton, StartCause,
        WindowEvent,
    },
    event_loop::{ControlFlow, EventLoopClosed, EventLoopWindowTarget as RootELW},
    monitor::MonitorHandle as RootMonitorHandle,
    window::{CursorIcon, WindowId as RootWindowId},
};

use super::{
    monitor::MonitorHandle,
    window::{WindowId, WindowRequest},
    DeviceId,
};

pub struct EventLoopWindowTarget<T> {
    /// Gtk application
    pub(crate) app: gtk::Application,
    /// Window Ids of the application
    pub(crate) windows: Rc<RefCell<HashSet<WindowId>>>,
    /// Window requests sender
    pub(crate) window_requests_tx: Sender<(WindowId, WindowRequest)>,
    /// Window requests receiver
    pub(crate) window_requests_rx: Receiver<(WindowId, WindowRequest)>,
    _marker: std::marker::PhantomData<T>,
}

impl<T> EventLoopWindowTarget<T> {
    #[inline]
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        todo!()
    }

    #[inline]
    pub fn primary_monitor(&self) -> Option<RootMonitorHandle> {
        todo!()
    }
}

pub struct EventLoop<T: 'static> {
    /// Window target.
    window_target: RootELW<T>,
    /// User event sender for EventLoopProxy
    user_event_tx: Sender<T>,
    /// User event receiver
    user_event_rx: Receiver<T>,
}

impl<T: 'static> EventLoop<T> {
    pub fn new() -> EventLoop<T> {
        assert_is_main_thread("new_any_thread");
        EventLoop::new_gtk().expect("Failed to initialize any backend!")
    }

    fn new_gtk() -> Result<EventLoop<T>, Box<dyn Error>> {
        let app = gtk::Application::new(Some("org.tauri"), gio::ApplicationFlags::empty())?;
        let cancellable: Option<&Cancellable> = None;
        app.register(cancellable)?;

        // Create event loop window target.
        let (window_requests_tx, window_requests_rx) = channel();
        let window_target = EventLoopWindowTarget {
            app,
            windows: Rc::new(RefCell::new(HashSet::new())),
            window_requests_tx,
            window_requests_rx,
            _marker: std::marker::PhantomData,
        };

        // Create user event channel
        let (user_event_tx, user_event_rx) = channel();

        // Create event loop itself.
        let event_loop = Self {
            window_target: RootELW {
                p: window_target,
                _marker: std::marker::PhantomData,
            },
            user_event_tx,
            user_event_rx,
        };

        Ok(event_loop)
    }

    #[inline]
    pub fn run<F>(self, callback: F) -> !
    where
        F: FnMut(Event<'_, T>, &RootELW<T>, &mut ControlFlow) + 'static,
    {
        self.run_return(callback);
        process::exit(0)
    }

    pub(crate) fn run_return<F>(self, mut callback: F)
    where
        F: FnMut(Event<'_, T>, &RootELW<T>, &mut ControlFlow) + 'static,
    {
        let mut control_flow = ControlFlow::default();
        let window_target = self.window_target;
        let (event_tx, event_rx) = channel::<Event<'_, T>>();

        // Send StartCause::Init event
        let tx_clone = event_tx.clone();
        window_target.p.app.connect_activate(move |_| {
            if let Err(e) = tx_clone.send(Event::NewEvents(StartCause::Init)) {
                log::warn!("Failed to send init event to event channel: {}", e);
            }
        });
        window_target.p.app.activate();

        let context = MainContext::default();
        context.push_thread_default();
        let keep_running = Rc::new(RefCell::new(true));
        let keep_running_ = keep_running.clone();
        let user_event_rx = self.user_event_rx;
        idle_add_local(move || {
            // User event
            if let Ok(event) = user_event_rx.try_recv() {
                if let Err(e) = event_tx.send(Event::UserEvent(event)) {
                    log::warn!("Failed to send user event to event channel: {}", e);
                }
            }

            // Widnow Request
            if let Ok((id, request)) = window_target.p.window_requests_rx.try_recv() {
                let window = window_target
                    .p
                    .app
                    .get_window_by_id(id.0)
                    .expect("Failed to send closed window event!");

                match request {
                    WindowRequest::Title(title) => window.set_title(&title),
                    WindowRequest::Position((x, y)) => window.move_(x, y),
                    WindowRequest::Size((w, h)) => window.resize(w, h),
                    WindowRequest::MinSize((min_width, min_height)) => window
                        .set_geometry_hints::<ApplicationWindow>(
                            None,
                            Some(&gdk::Geometry {
                                min_width,
                                min_height,
                                max_width: 0,
                                max_height: 0,
                                base_width: 0,
                                base_height: 0,
                                width_inc: 0,
                                height_inc: 0,
                                min_aspect: 0f64,
                                max_aspect: 0f64,
                                win_gravity: gdk::Gravity::Center,
                            }),
                            gdk::WindowHints::MIN_SIZE,
                        ),
                    WindowRequest::MaxSize((max_width, max_height)) => window
                        .set_geometry_hints::<ApplicationWindow>(
                            None,
                            Some(&gdk::Geometry {
                                min_width: 0,
                                min_height: 0,
                                max_width,
                                max_height,
                                base_width: 0,
                                base_height: 0,
                                width_inc: 0,
                                height_inc: 0,
                                min_aspect: 0f64,
                                max_aspect: 0f64,
                                win_gravity: gdk::Gravity::Center,
                            }),
                            gdk::WindowHints::MAX_SIZE,
                        ),
                    WindowRequest::Visible(visible) => {
                        if visible {
                            window.show();
                        } else {
                            window.hide();
                        }
                    }
                    WindowRequest::Resizable(resizable) => window.set_resizable(resizable),
                    WindowRequest::Minimized(minimized) => {
                        if minimized {
                            window.iconify();
                        } else {
                            window.deiconify();
                        }
                    }
                    WindowRequest::Maximized(maximized) => {
                        if maximized {
                            window.maximize();
                        } else {
                            window.unmaximize();
                        }
                    }
                    WindowRequest::DragWindow => {
                        let display = window.get_display();
                        if let Some(cursor) = display
                            .get_device_manager()
                            .and_then(|device_manager| device_manager.get_client_pointer())
                        {
                            let (_, x, y) = cursor.get_position();
                            window.begin_move_drag(1, x, y, 0);
                        }
                    }
                    WindowRequest::Fullscreen(fullscreen) => match fullscreen {
                        Some(_) => window.fullscreen(),
                        None => window.unfullscreen(),
                    },
                    WindowRequest::Decorations(decorations) => window.set_decorated(decorations),
                    WindowRequest::AlwaysOnTop(always_on_top) => {
                        window.set_keep_above(always_on_top)
                    }
                    WindowRequest::WindowIcon(window_icon) => {
                        if let Some(icon) = window_icon {
                            window.set_icon(Some(&icon.inner.into()));
                        }
                    }
                    WindowRequest::UserAttention(request_type) => {
                        if request_type.is_some() {
                            window.set_urgency_hint(true)
                        }
                    }
                    WindowRequest::SkipTaskbar => window.set_skip_taskbar_hint(true),
                    WindowRequest::CursorIcon(cursor) => {
                        if let Some(gdk_window) = window.get_window() {
                            let display = window.get_display();
                            match cursor {
                                Some(cr) => gdk_window.set_cursor(
                                    Cursor::from_name(
                                        &display,
                                        match cr {
                                            CursorIcon::Crosshair => "crosshair",
                                            CursorIcon::Hand => "pointer",
                                            CursorIcon::Arrow => "crosshair",
                                            CursorIcon::Move => "move",
                                            CursorIcon::Text => "text",
                                            CursorIcon::Wait => "wait",
                                            CursorIcon::Help => "help",
                                            CursorIcon::Progress => "progress",
                                            CursorIcon::NotAllowed => "not-allowed",
                                            CursorIcon::ContextMenu => "context-menu",
                                            CursorIcon::Cell => "cell",
                                            CursorIcon::VerticalText => "vertical-text",
                                            CursorIcon::Alias => "alias",
                                            CursorIcon::Copy => "copy",
                                            CursorIcon::NoDrop => "no-drop",
                                            CursorIcon::Grab => "grab",
                                            CursorIcon::Grabbing => "grabbing",
                                            CursorIcon::AllScroll => "all-scroll",
                                            CursorIcon::ZoomIn => "zoom-in",
                                            CursorIcon::ZoomOut => "zoom-out",
                                            CursorIcon::EResize => "e-resize",
                                            CursorIcon::NResize => "n-resize",
                                            CursorIcon::NeResize => "ne-resize",
                                            CursorIcon::NwResize => "nw-resize",
                                            CursorIcon::SResize => "s-resize",
                                            CursorIcon::SeResize => "se-resize",
                                            CursorIcon::SwResize => "sw-resize",
                                            CursorIcon::WResize => "w-resize",
                                            CursorIcon::EwResize => "ew-resize",
                                            CursorIcon::NsResize => "ns-resize",
                                            CursorIcon::NeswResize => "nesw-resize",
                                            CursorIcon::NwseResize => "nwse-resize",
                                            CursorIcon::ColResize => "col-resize",
                                            CursorIcon::RowResize => "row-resize",
                                            CursorIcon::Default => "default",
                                        },
                                    )
                                    .as_ref(),
                                ),
                                None => gdk_window.set_cursor(Some(&Cursor::new_for_display(
                                    &display,
                                    CursorType::BlankCursor,
                                ))),
                            }
                        };
                    }
                    WindowRequest::WireUpEvents => {
                        let windows_rc = window_target.p.windows.clone();
                        let tx_clone = event_tx.clone();

                        window.connect_delete_event(move |_, _| {
                            windows_rc.borrow_mut().remove(&id);
                            if let Err(e) = tx_clone.send(Event::WindowEvent {
                                window_id: RootWindowId(id),
                                event: WindowEvent::CloseRequested,
                            }) {
                                log::warn!(
                                    "Failed to send window close event to event channel: {}",
                                    e
                                );
                            }
                            Inhibit(false)
                        });

                        let tx_clone = event_tx.clone();
                        window.connect_configure_event(move |_, event| {
                            let (x, y) = event.get_position();
                            if let Err(e) = tx_clone.send(Event::WindowEvent {
                                window_id: RootWindowId(id),
                                event: WindowEvent::Moved(PhysicalPosition::new(x, y)),
                            }) {
                                log::warn!(
                                    "Failed to send window moved event to event channel: {}",
                                    e
                                );
                            }

                            let (w, h) = event.get_size();
                            if let Err(e) = tx_clone.send(Event::WindowEvent {
                                window_id: RootWindowId(id),
                                event: WindowEvent::Resized(PhysicalSize::new(w, h)),
                            }) {
                                log::warn!(
                                    "Failed to send window resized event to event channel: {}",
                                    e
                                );
                            }
                            false
                        });

                        let tx_clone = event_tx.clone();
                        window.connect_window_state_event(move |_window, event| {
                            let state = event.get_new_window_state();

                            if let Err(e) = tx_clone.send(Event::WindowEvent {
                                window_id: RootWindowId(id),
                                event: WindowEvent::Focused(state.contains(WindowState::FOCUSED)),
                            }) {
                                log::warn!(
                                    "Failed to send window focused event to event channel: {}",
                                    e
                                );
                            }
                            Inhibit(false)
                        });

                        let tx_clone = event_tx.clone();
                        window.connect_destroy_event(move |_, _| {
                            if let Err(e) = tx_clone.send(Event::WindowEvent {
                                window_id: RootWindowId(id),
                                event: WindowEvent::Destroyed,
                            }) {
                                log::warn!(
                                    "Failed to send window destroyed event to event channel: {}",
                                    e
                                );
                            }
                            Inhibit(false)
                        });

                        let tx_clone = event_tx.clone();
                        window.connect_enter_notify_event(move |_, _| {
                            if let Err(e) = tx_clone.send(Event::WindowEvent {
                                window_id: RootWindowId(id),
                                event: WindowEvent::CursorEntered {
                                    // FIXME: currently we use a dummy device id, find if we can get device id from gtk
                                    device_id: RootDeviceId(DeviceId(0)),
                                },
                            }) {
                                log::warn!(
                                    "Failed to send cursor entered event to event channel: {}",
                                    e
                                );
                            }
                            Inhibit(false)
                        });

                        let tx_clone = event_tx.clone();
                        window.connect_motion_notify_event(move |window, _| {
                            let display = window.get_display();
                            if let Some(cursor) = display
                                .get_device_manager()
                                .and_then(|device_manager| device_manager.get_client_pointer())
                            {
                                let (_, x, y) = cursor.get_position();
                                if let Err(e) = tx_clone.send(Event::WindowEvent {
                                    window_id: RootWindowId(id),
                                    event: WindowEvent::CursorMoved {
                                        position: PhysicalPosition::new(x as f64, y as f64),
                                        // FIXME: currently we use a dummy device id, find if we can get device id from gtk
                                        device_id: RootDeviceId(DeviceId(0)),
                                        // this field is depracted so it is fine to pass empty state
                                        modifiers: ModifiersState::empty(),
                                    },
                                }) {
                                    log::warn!(
                                        "Failed to send cursor moved event to event channel: {}",
                                        e
                                    );
                                }
                            }
                            Inhibit(false)
                        });

                        let tx_clone = event_tx.clone();
                        window.connect_leave_notify_event(move |_, _| {
                            if let Err(e) = tx_clone.send(Event::WindowEvent {
                                window_id: RootWindowId(id),
                                event: WindowEvent::CursorLeft {
                                    // FIXME: currently we use a dummy device id, find if we can get device id from gtk
                                    device_id: RootDeviceId(DeviceId(0)),
                                },
                            }) {
                                log::warn!(
                                    "Failed to send cursor left event to event channel: {}",
                                    e
                                );
                            }
                            Inhibit(false)
                        });

                        let tx_clone = event_tx.clone();
                        window.connect_button_press_event(move |_, event| {
                            let button = event.get_button();
                            if let Err(e) = tx_clone.send(Event::WindowEvent {
                                window_id: RootWindowId(id),
                                event: WindowEvent::MouseInput {
                                    button: match button {
                                        1 => MouseButton::Left,
                                        2 => MouseButton::Middle,
                                        3 => MouseButton::Right,
                                        _ => MouseButton::Other(button as u16),
                                    },
                                    state: ElementState::Pressed,
                                    // FIXME: currently we use a dummy device id, find if we can get device id from gtk
                                    device_id: RootDeviceId(DeviceId(0)),
                                    // this field is depracted so it is fine to pass empty state
                                    modifiers: ModifiersState::empty(),
                                },
                            }) {
                                log::warn!(
                                    "Failed to send mouse input preseed event to event channel: {}",
                                    e
                                );
                            }
                            Inhibit(false)
                        });

                        let tx_clone = event_tx.clone();
                        window.connect_button_release_event(move |_, event| {
                            let button = event.get_button();
                            if let Err(e) = tx_clone.send(Event::WindowEvent {
                                window_id: RootWindowId(id),
                                event: WindowEvent::MouseInput {
                                    button: match button {
                                        1 => MouseButton::Left,
                                        2 => MouseButton::Middle,
                                        3 => MouseButton::Right,
                                        _ => MouseButton::Other(button as u16),
                                    },
                                    state: ElementState::Released,
                                    // FIXME: currently we use a dummy device id, find if we can get device id from gtk
                                    device_id: RootDeviceId(DeviceId(0)),
                                    // this field is depracted so it is fine to pass empty state
                                    modifiers: ModifiersState::empty(),
                                },
                            }) {
                                log::warn!(
                  "Failed to send mouse input released event to event channel: {}",
                  e
                );
                            }
                            Inhibit(false)
                        });
                    }
                    WindowRequest::Redraw => window.queue_draw(),
                }
            }

            // Event control flow
            match control_flow {
                ControlFlow::Exit => {
                    keep_running_.replace(false);
                    Continue(false)
                }
                // TODO better control flow handling
                _ => {
                    if let Ok(event) = event_rx.try_recv() {
                        callback(event, &window_target, &mut control_flow);
                    } else {
                        callback(Event::MainEventsCleared, &window_target, &mut control_flow);
                    }
                    Continue(true)
                }
            }
        });
        context.pop_thread_default();

        while *keep_running.borrow() {
            gtk::main_iteration();
        }
    }

    #[inline]
    pub fn window_target(&self) -> &RootELW<T> {
        &self.window_target
    }

    /// Creates an `EventLoopProxy` that can be used to dispatch user events to the main event loop.
    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            user_event_tx: self.user_event_tx.clone(),
        }
    }
}

/// Used to send custom events to `EventLoop`.
#[derive(Debug)]
pub struct EventLoopProxy<T: 'static> {
    user_event_tx: Sender<T>,
}

impl<T: 'static> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        Self {
            user_event_tx: self.user_event_tx.clone(),
        }
    }
}

impl<T: 'static> EventLoopProxy<T> {
    /// Send an event to the `EventLoop` from which this proxy was created. This emits a
    /// `UserEvent(event)` event in the event loop, where `event` is the value passed to this
    /// function.
    ///
    /// Returns an `Err` if the associated `EventLoop` no longer exists.
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        self.user_event_tx
            .send(event)
            .map_err(|SendError(error)| EventLoopClosed(error))
    }
}

fn assert_is_main_thread(suggested_method: &str) {
    if !is_main_thread() {
        panic!(
            "Initializing the event loop outside of the main thread is a significant \
             cross-platform compatibility hazard. If you really, absolutely need to create an \
             EventLoop on a different thread, please use the `EventLoopExtUnix::{}` function.",
            suggested_method
        );
    }
}

#[cfg(target_os = "linux")]
fn is_main_thread() -> bool {
    use libc::{c_long, getpid, syscall, SYS_gettid};

    unsafe { syscall(SYS_gettid) == getpid() as c_long }
}

#[cfg(any(target_os = "dragonfly", target_os = "freebsd", target_os = "openbsd"))]
fn is_main_thread() -> bool {
    use libc::pthread_main_np;

    unsafe { pthread_main_np() == 1 }
}

#[cfg(target_os = "netbsd")]
fn is_main_thread() -> bool {
    std::thread::current().name() == Some("main")
}
