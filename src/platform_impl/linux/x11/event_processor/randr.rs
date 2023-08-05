//! Handles `randr` events.

use super::prelude::*;

use super::super::monitor;
use std::sync::{Arc, Mutex};
use x11rb::protocol::randr;

impl EventProcessor {
    /// Handle the `RandrNotify` event.
    fn handle_randr_notify(
        &mut self,
        wt: &WindowTarget,
        _xev: randr::NotifyEvent,
        callback: &mut dyn FnMut(Event<Infallible>),
    ) {
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
                                let inner_size =
                                    Arc::new(Mutex::new(PhysicalSize::new(new_width, new_height)));
                                callback(Event::WindowEvent {
                                    window_id,
                                    event: WindowEvent::ScaleFactorChanged {
                                        scale_factor: new_monitor.scale_factor,
                                        inner_size_writer: InnerSizeWriter::new(Arc::downgrade(
                                            &inner_size,
                                        )),
                                    },
                                });

                                let new_inner_size = *inner_size.lock().unwrap();
                                drop(inner_size);

                                if new_inner_size != old_inner_size {
                                    let (new_width, new_height) = new_inner_size.into();
                                    window.request_inner_size_physical(new_width, new_height);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

event_handlers! {
    EventCode::Extension {
        extension: randr::X11_EXTENSION_NAME,
        offset: randr::NOTIFY_EVENT,
    } => EventProcessor::handle_randr_notify,
}
