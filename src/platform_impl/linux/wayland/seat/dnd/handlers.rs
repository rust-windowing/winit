use std::{
    io::{self, BufRead, BufReader},
    path::PathBuf,
};

use percent_encoding::percent_decode_str;
use sctk::data_device::{DataOffer, DndEvent};
use wayland_client::Display;

use crate::{
    dpi::PhysicalPosition,
    event::WindowEvent,
    platform_impl::wayland::{event_loop::WinitState, make_wid, DeviceId},
};

use super::DndInner;

const MIME_TYPE: &str = "text/uri-list";

pub(super) fn handle_dnd(event: DndEvent<'_>, inner: &mut DndInner, winit_state: &mut WinitState) {
    match event {
        DndEvent::Enter {
            offer: Some(offer),
            surface,
            x,
            y,
            ..
        } => {
            let window_id = make_wid(&surface);

            if let Ok(paths) = parse_offer(&winit_state.display, offer) {
                if !paths.is_empty() {
                    offer.accept(Some(MIME_TYPE.into()));

                    winit_state.event_sink.push_window_event(
                        WindowEvent::CursorEntered {
                            device_id: crate::event::DeviceId(
                                crate::platform_impl::DeviceId::Wayland(DeviceId),
                            ),
                        },
                        window_id,
                    );
                    winit_state.event_sink.push_window_event(
                        WindowEvent::CursorMoved {
                            device_id: crate::event::DeviceId(
                                crate::platform_impl::DeviceId::Wayland(DeviceId),
                            ),
                            position: PhysicalPosition::new(x, y),
                            modifiers: Default::default(),
                        },
                        window_id,
                    );

                    for path in paths {
                        winit_state
                            .event_sink
                            .push_window_event(WindowEvent::HoveredFile(path), window_id);
                    }
                    inner.window_id = Some(window_id);
                }
            }
        }
        DndEvent::Drop { offer: Some(offer) } => {
            if let Some(window_id) = inner.window_id {
                inner.window_id = None;

                if let Ok(paths) = parse_offer(&winit_state.display, offer) {
                    for path in paths {
                        winit_state
                            .event_sink
                            .push_window_event(WindowEvent::DroppedFile(path), window_id);
                    }
                }
            }
        }
        DndEvent::Leave => {
            if let Some(window_id) = inner.window_id {
                inner.window_id = None;

                winit_state
                    .event_sink
                    .push_window_event(WindowEvent::HoveredFileCancelled, window_id);
                winit_state.event_sink.push_window_event(
                    WindowEvent::CursorLeft {
                        device_id: crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(
                            DeviceId,
                        )),
                    },
                    window_id,
                );
            }
        }
        DndEvent::Motion { x, y, .. } => {
            if let Some(window_id) = inner.window_id {
                winit_state.event_sink.push_window_event(
                    WindowEvent::CursorMoved {
                        device_id: crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(
                            DeviceId,
                        )),
                        position: PhysicalPosition::new(x, y),
                        modifiers: Default::default(),
                    },
                    window_id,
                );
            }
        }
        _ => {}
    }
}

fn parse_offer(display: &Display, offer: &DataOffer) -> io::Result<Vec<PathBuf>> {
    let can_accept = offer.with_mime_types(|types| types.iter().any(|s| s == MIME_TYPE));
    if can_accept {
        // Format: https://www.iana.org/assignments/media-types/text/uri-list
        let mut paths = Vec::new();
        let pipe = offer.receive(MIME_TYPE.into())?;
        let _ = display.flush();
        for line in BufReader::new(pipe).lines() {
            let line = line?;
            if line.starts_with('#') {
                continue;
            }

            let decoded = match percent_decode_str(&line).decode_utf8() {
                Ok(decoded) => decoded,
                Err(_) => continue,
            };
            if let Some(path) = decoded.strip_prefix("file://") {
                paths.push(PathBuf::from(path));
            }
        }
        Ok(paths)
    } else {
        Ok(Vec::new())
    }
}
