//! Handles the `ClientMessage` event.

use super::super::DndState;
use super::prelude::*;

impl EventProcessor {
    fn handle_client_message(
        &mut self,
        wt: &WindowTarget,
        client_msg: xproto::ClientMessageEvent,
        callback: &mut dyn FnMut(Event<Infallible>),
    ) {
        let atoms = wt.xconn.atoms();
        let window = client_msg.window;
        let window_id = mkwid(window);
        let data = client_msg.data.as_data32();

        if data[0] == wt.wm_delete_window {
            callback(Event::WindowEvent {
                window_id,
                event: WindowEvent::CloseRequested,
            });
        } else if data[0] == wt.net_wm_ping {
            wt.xconn
                .xcb_connection()
                .send_event(
                    false,
                    wt.root,
                    xproto::EventMask::SUBSTRUCTURE_NOTIFY
                        | xproto::EventMask::SUBSTRUCTURE_REDIRECT,
                    client_msg,
                )
                .expect_then_ignore_error("Failed to send `ClientMessage` event.");
        } else if client_msg.type_ == atoms[XdndEnter] {
            let source_window = data[0] as xproto::Window;
            let flags = data[1];
            let version = flags >> 24;
            self.dnd.version = Some(version);
            let has_more_types = flags - (flags & (u32::max_value() - 1)) == 1;
            if !has_more_types {
                let type_list = vec![data[2], data[3], data[4]];
                self.dnd.type_list = Some(type_list);
            } else if let Ok(more_types) = unsafe { self.dnd.get_type_list(source_window) } {
                self.dnd.type_list = Some(more_types);
            }
        } else if client_msg.type_ == atoms[XdndPosition] {
            // This event occurs every time the mouse moves while a file's being dragged
            // over our window. We emit HoveredFile in response; while the macOS backend
            // does that upon a drag entering, XDND doesn't have access to the actual drop
            // data until this event. For parity with other platforms, we only emit
            // `HoveredFile` the first time, though if winit's API is later extended to
            // supply position updates with `HoveredFile` or another event, implementing
            // that here would be trivial.

            let source_window = data[0];

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
                type_list.contains(&atoms[TextUriList])
            } else {
                false
            };

            if accepted {
                self.dnd.source_window = Some(source_window);
                unsafe {
                    if self.dnd.result.is_none() {
                        let time = if version >= 1 {
                            data[3] as xproto::Timestamp
                        } else {
                            // In version 0, time isn't specified
                            x11rb::CURRENT_TIME
                        };

                        // Log this timestamp.
                        wt.xconn.set_timestamp(time);

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
        } else if client_msg.type_ == atoms[XdndDrop] {
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
                let source_window = data[0] as xproto::Window;
                (source_window, DndState::Rejected)
            };
            unsafe {
                self.dnd
                    .send_finished(window, source_window, state)
                    .expect("Failed to send `XdndFinished` message.");
            }
            self.dnd.reset();
        } else if client_msg.type_ == atoms[XdndLeave] {
            self.dnd.reset();
            callback(Event::WindowEvent {
                window_id,
                event: WindowEvent::HoveredFileCancelled,
            });
        }
    }
}

event_handlers! {
    xp_code(xproto::CLIENT_MESSAGE_EVENT) => EventProcessor::handle_client_message,
}
