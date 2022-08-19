use std::io::{self, Read};
use std::os::unix::prelude::{AsRawFd, RawFd};
use std::path::PathBuf;
use std::str;

use percent_encoding::percent_decode_str;
use sctk::data_device::{DataOffer, DndEvent, ReadPipe};
use sctk::reexports::calloop::{generic::Generic, Interest, LoopHandle, Mode, PostAction};

use crate::dpi::PhysicalPosition;
use crate::event::WindowEvent;
use crate::platform_impl::wayland::{event_loop::WinitState, make_wid, DeviceId};

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
            inner.window_id = Some(window_id);
            offer.accept(Some(MIME_TYPE.into()));
            let _ = parse_offer(&inner.loop_handle, offer, move |paths, winit_state| {
                if !paths.is_empty() {
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
                }
            });
        }
        DndEvent::Drop { offer: Some(offer) } => {
            if let Some(window_id) = inner.window_id {
                inner.window_id = None;

                let _ = parse_offer(&inner.loop_handle, offer, move |paths, winit_state| {
                    for path in paths {
                        winit_state
                            .event_sink
                            .push_window_event(WindowEvent::DroppedFile(path), window_id);
                    }
                });
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

fn parse_offer(
    loop_handle: &LoopHandle<'static, WinitState>,
    offer: &DataOffer,
    mut callback: impl FnMut(Vec<PathBuf>, &mut WinitState) + 'static,
) -> io::Result<()> {
    let can_accept = offer.with_mime_types(|types| types.iter().any(|s| s == MIME_TYPE));
    if can_accept {
        let pipe = offer.receive(MIME_TYPE.into())?;
        read_pipe_nonblocking(pipe, loop_handle, move |bytes, winit_state| {
            // Format: https://www.iana.org/assignments/media-types/text/uri-list
            let mut paths = Vec::new();
            for line in bytes.split(|b| *b == b'\n') {
                let line = match str::from_utf8(line) {
                    Ok(line) => line,
                    Err(_) => continue,
                };

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
            callback(paths, winit_state);
        })?;
    }
    Ok(())
}

fn read_pipe_nonblocking(
    pipe: ReadPipe,
    loop_handle: &LoopHandle<'static, WinitState>,
    mut callback: impl FnMut(Vec<u8>, &mut WinitState) + 'static,
) -> io::Result<()> {
    unsafe {
        make_fd_nonblocking(pipe.as_raw_fd())?;
    }

    let mut content = Vec::<u8>::with_capacity(u16::MAX as usize);
    let mut reader_buffer = [0; u16::MAX as usize];
    let reader = Generic::new(pipe, Interest::READ, Mode::Level);

    let _ = loop_handle.insert_source(reader, move |_, file, winit_state| {
        let action = loop {
            match file.read(&mut reader_buffer) {
                Ok(0) => {
                    let data = std::mem::take(&mut content);
                    callback(data, winit_state);
                    break PostAction::Remove;
                }
                Ok(n) => content.extend_from_slice(&reader_buffer[..n]),
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => break PostAction::Continue,
                Err(_) => break PostAction::Remove,
            }
        };

        Ok(action)
    });
    Ok(())
}

unsafe fn make_fd_nonblocking(raw_fd: RawFd) -> io::Result<()> {
    let flags = libc::fcntl(raw_fd, libc::F_GETFL);
    if flags < 0 {
        return Err(io::Error::from_raw_os_error(flags));
    }
    let result = libc::fcntl(raw_fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    if result < 0 {
        return Err(io::Error::from_raw_os_error(result));
    }

    Ok(())
}
