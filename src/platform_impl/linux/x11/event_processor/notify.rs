//! Handles the `*Notify` family of events, as well as `Expose`.

use super::prelude::*;

use std::sync::{Arc, Mutex};

impl EventProcessor {
    /// Handle the `SelectionNotify` event.
    fn handle_selection_notify(
        &mut self,
        wt: &WindowTarget,
        xsel: xproto::SelectionNotifyEvent,
        callback: &mut dyn FnMut(Event<Infallible>),
    ) {
        let atoms = wt.xconn.atoms();
        let window = xsel.requestor;
        let window_id = mkwid(window);

        // Set the timestamp.
        wt.xconn.set_timestamp(xsel.time as xproto::Timestamp);

        if xsel.property == atoms[XdndSelection] {
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

    /// Handle the `ConfigureNotify` event.
    fn handle_configure_notify(
        &mut self,
        wt: &WindowTarget,
        xev: xproto::ConfigureNotifyEvent,
        callback: &mut dyn FnMut(Event<Infallible>),
    ) {
        let xwindow = xev.window;
        let window_id = mkwid(xwindow);

        if let Some(window) = self.with_window(wt, xwindow, Arc::clone) {
            // So apparently...
            // `XSendEvent` (synthetic `ConfigureNotify`) -> position relative to root
            // `XConfigureNotify` (real `ConfigureNotify`) -> position relative to parent
            // https://tronche.com/gui/x/icccm/sec-4.html#s-4.1.5
            // We don't want to send `Moved` when this is false, since then every `Resized`
            // (whether the window moved or not) is accompanied by an extraneous `Moved` event
            // that has a position relative to the parent window.
            let is_synthetic = xev.response_type == xproto::CONFIGURE_NOTIFY_EVENT;

            // These are both in physical space.
            let new_inner_size = (xev.width as u32, xev.height as u32);
            let new_inner_position = (xev.x as i32, xev.y as i32);

            let (mut resized, moved) = {
                let mut shared_state_lock = window.shared_state_lock();

                let resized = util::maybe_change(&mut shared_state_lock.size, new_inner_size);
                let moved = if is_synthetic {
                    util::maybe_change(&mut shared_state_lock.inner_position, new_inner_position)
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
                        let frame_extents = wt.xconn.get_frame_extents_heuristic(xwindow, wt.root);
                        shared_state_lock.frame_extents = Some(frame_extents.clone());
                        frame_extents
                    });
                let outer =
                    frame_extents.inner_pos_to_outer(new_inner_position.0, new_inner_position.1);
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
                    let new_inner_size = PhysicalSize::new(new_width, new_height);

                    // Unlock shared state to prevent deadlock in callback below
                    drop(shared_state_lock);

                    let inner_size = Arc::new(Mutex::new(new_inner_size));
                    callback(Event::WindowEvent {
                        window_id,
                        event: WindowEvent::ScaleFactorChanged {
                            scale_factor: new_scale_factor,
                            inner_size_writer: InnerSizeWriter::new(Arc::downgrade(&inner_size)),
                        },
                    });

                    let new_inner_size = *inner_size.lock().unwrap();
                    drop(inner_size);

                    if new_inner_size != old_inner_size {
                        window.request_inner_size_physical(
                            new_inner_size.width,
                            new_inner_size.height,
                        );
                        window.shared_state_lock().dpi_adjusted = Some(new_inner_size.into());
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
                    window.request_inner_size_physical(adjusted_size.0, adjusted_size.1);
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

    /// Handle the `ReparentNotify` event.
    fn handle_reparent_notify(
        &mut self,
        wt: &WindowTarget,
        xev: xproto::ReparentNotifyEvent,
        _callback: &mut dyn FnMut(Event<Infallible>),
    ) {
        // This is generally a reliable way to detect when the window manager's been
        // replaced, though this event is only fired by reparenting window managers
        // (which is almost all of them). Failing to correctly update WM info doesn't
        // really have much impact, since on the WMs affected (xmonad, dwm, etc.) the only
        // effect is that we waste some time trying to query unsupported properties.
        wt.xconn.update_cached_wm_info(wt.root);

        self.with_window(wt, xev.window, |window| {
            window.invalidate_cached_frame_extents();
        });
    }

    /// Handle the `MapNotify` event.
    fn handle_map_notify(
        &mut self,
        wt: &WindowTarget,
        xev: xproto::MapNotifyEvent,
        callback: &mut dyn FnMut(Event<Infallible>),
    ) {
        let window = xev.window;
        let window_id = mkwid(window);

        // XXX re-issue the focus state when mapping the window.
        //
        // The purpose of it is to deliver initial focused state of the newly created
        // window, given that we can't rely on `CreateNotify`, due to it being not
        // sent.
        let focus = self
            .with_window(wt, window, |window| window.has_focus())
            .unwrap_or_default();
        callback(Event::WindowEvent {
            window_id,
            event: WindowEvent::Focused(focus),
        });
    }

    /// Handle the `DestroyNotify` event.
    fn handle_destroy_notify(
        &mut self,
        wt: &WindowTarget,
        xev: xproto::DestroyNotifyEvent,
        callback: &mut dyn FnMut(Event<Infallible>),
    ) {
        let window = xev.window;
        let window_id = mkwid(window);

        // In the event that the window's been destroyed without being dropped first, we
        // cleanup again here.
        wt.windows.borrow_mut().remove(&WindowId(window as _));

        // Since all XIM stuff needs to happen from the same thread, we destroy the input
        // context here instead of when dropping the window.
        wt.ime
            .borrow_mut()
            .remove_context(window as ffi::Window)
            .expect("Failed to destroy input context");

        callback(Event::WindowEvent {
            window_id,
            event: WindowEvent::Destroyed,
        });
    }

    /// Handle the `VisibilityNotify` event.
    fn handle_visibility_notify(
        &mut self,
        wt: &WindowTarget,
        xev: xproto::VisibilityNotifyEvent,
        callback: &mut dyn FnMut(Event<Infallible>),
    ) {
        let xwindow = xev.window as xproto::Window;
        callback(Event::WindowEvent {
            window_id: mkwid(xwindow),
            event: WindowEvent::Occluded(matches!(xev.state, xproto::Visibility::FULLY_OBSCURED)),
        });
        self.with_window(wt, xwindow, |window| {
            window.visibility_notify();
        });
    }

    /// Handle the `Expose` event.
    fn handle_expose(
        &mut self,
        _wt: &WindowTarget,
        xev: xproto::ExposeEvent,
        callback: &mut dyn FnMut(Event<Infallible>),
    ) {
        // Multiple Expose events may be received for subareas of a window.
        // We issue `RedrawRequested` only for the last event of such a series.
        if xev.count == 0 {
            let window = xev.window;
            let window_id = mkwid(window);

            callback(Event::RedrawRequested(window_id));
        }
    }
}

event_handlers! {
    xp_code(xproto::SELECTION_NOTIFY_EVENT) => EventProcessor::handle_selection_notify,
    xp_code(xproto::CONFIGURE_NOTIFY_EVENT) => EventProcessor::handle_configure_notify,
    xp_code(xproto::REPARENT_NOTIFY_EVENT) => EventProcessor::handle_reparent_notify,
    xp_code(xproto::MAP_NOTIFY_EVENT) => EventProcessor::handle_map_notify,
    xp_code(xproto::DESTROY_NOTIFY_EVENT) => EventProcessor::handle_destroy_notify,
    xp_code(xproto::VISIBILITY_NOTIFY_EVENT) => EventProcessor::handle_visibility_notify,
    xp_code(xproto::EXPOSE_EVENT) => EventProcessor::handle_expose,
}
