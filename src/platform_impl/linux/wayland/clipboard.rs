//! Wayland clipboard handling.

use std::collections::HashSet;
use std::fs::File;
use std::io::{Error, ErrorKind, Read, Result, Write};
use std::os::unix::io::{FromRawFd, IntoRawFd, RawFd};
use std::rc::Rc;
use std::sync::Arc;

use sctk::reexports::calloop::generic::Generic;
use sctk::reexports::calloop::{Interest, LoopHandle, Mode, PostAction};

use sctk::data_device::{DataSourceEvent, ReadPipe, WritePipe};
use sctk::environment::Environment;
use sctk::primary_selection::PrimarySelectionSourceEvent;

use crate::event::{ClipboardContent, ClipboardMetadata, WindowEvent};
use crate::platform_impl::wayland::env::WinitEnv;
use crate::platform_impl::wayland::event_loop::WinitState;
use crate::platform_impl::wayland::window::shim::LatestSeat;

use super::WindowId;

#[derive(Clone, Copy)]
pub enum ClipboardType {
    /// Primary clipboard which is provided by `gtk_primary_selection_device_manager` or
    /// `zwp_primary_selection_device_manager` protocols.
    Primary,

    /// Clipboard which is provided by `wl_data_device_manager`.
    Clipboard,
}

pub struct ClipboardManager {
    /// Environment to access clipboard related globals.
    env: Environment<WinitEnv>,

    /// Loop handle which is used to register pipes in main event loop.
    loop_handle: LoopHandle<'static, WinitState>,

    /// Latest seat information.
    seat: Option<LatestSeat>,
}

impl ClipboardManager {
    pub fn new(env: Environment<WinitEnv>, loop_handle: LoopHandle<'static, WinitState>) -> Self {
        Self {
            env,
            loop_handle,
            seat: None,
        }
    }

    /// Update information about the latest observed seat and its serial.
    pub fn update_seat_info(&mut self, seat: Option<LatestSeat>) {
        self.seat = seat;
    }

    /// Set `ty` clipboard content to `content` and offering it with `mimes` mime types.
    pub fn set_content(
        &self,
        ty: ClipboardType,
        content: Rc<dyn AsRef<[u8]>>,
        mimes: HashSet<String>,
    ) {
        let seat = match self.seat.as_ref() {
            Some(seat) => seat,
            None => return,
        };

        let mimes = mimes.into_iter().collect();
        match ty {
            ClipboardType::Clipboard => self.set_clipboard_content(seat, content, mimes),
            ClipboardType::Primary => self.set_primary_content(seat, content, mimes),
        }
    }

    fn set_clipboard_content(
        &self,
        seat: &LatestSeat,
        content: Rc<dyn AsRef<[u8]>>,
        mimes: Vec<String>,
    ) {
        let loop_handle = self.loop_handle.clone();
        let data_source = self.env.new_data_source(mimes, move |event, _| {
            let pipe = match event {
                DataSourceEvent::Send { pipe, .. } => pipe,
                _ => return,
            };

            write_clipboard(pipe, content.clone(), &loop_handle);
        });

        let _ = self.env.with_data_device(&seat.seat, |device| {
            device.set_selection(&Some(data_source), seat.serial);
        });
    }

    fn set_primary_content(
        &self,
        seat: &LatestSeat,
        content: Rc<dyn AsRef<[u8]>>,
        mimes: Vec<String>,
    ) {
        let loop_handle = self.loop_handle.clone();
        let primary_source = self
            .env
            .new_primary_selection_source(mimes, move |event, _| {
                let pipe = match event {
                    PrimarySelectionSourceEvent::Send { pipe, .. } => pipe,
                    _ => return,
                };

                write_clipboard(pipe, content.clone(), &loop_handle);
            });

        let _ = self.env.with_primary_selection(&seat.seat, |device| {
            device.set_selection(&Some(primary_source), seat.serial);
        });
    }

    /// Request `ty` clipboard content picking from `mimes` forwarding it to `window_id` passing
    /// `metadata`.
    pub fn request_content(
        &self,
        window_id: WindowId,
        ty: ClipboardType,
        mimes: HashSet<String>,
        metadata: Option<Arc<ClipboardMetadata>>,
    ) {
        let seat = match self.seat.as_ref() {
            Some(seat) => seat,
            None => return,
        };

        match ty {
            ClipboardType::Primary => {
                self.request_primary_content(seat, window_id, mimes, metadata)
            }
            ClipboardType::Clipboard => {
                self.request_clipboard_content(seat, window_id, mimes, metadata)
            }
        }
    }

    fn request_clipboard_content(
        &self,
        seat: &LatestSeat,
        window_id: WindowId,
        mimes: HashSet<String>,
        metadata: Option<Arc<ClipboardMetadata>>,
    ) {
        let loop_handle = self.loop_handle.clone();
        let _ = self.env.with_data_device(&seat.seat, move |device| {
            device.with_selection(move |offer| {
                let offer = match offer {
                    Some(offer) => offer,
                    None => return,
                };

                let mut mime = String::new();
                offer.with_mime_types(|types| {
                    for ty in types {
                        if mimes.contains(ty) {
                            mime = ty.to_string();
                        }
                    }
                });

                let reader = match offer.receive(mime.clone()) {
                    Ok(reader) => reader,
                    Err(_) => return,
                };

                read_clipboard(reader, mime, metadata, window_id, &loop_handle);
            })
        });
    }

    fn request_primary_content(
        &self,
        seat: &LatestSeat,
        window_id: WindowId,
        mimes: HashSet<String>,
        metadata: Option<Arc<ClipboardMetadata>>,
    ) {
        let loop_handle = self.loop_handle.clone();
        let _ = self.env.with_primary_selection(&seat.seat, move |device| {
            device.with_selection(move |offer| {
                let offer = match offer {
                    Some(offer) => offer,
                    None => return,
                };

                let mut mime = String::new();
                offer.with_mime_types(|types| {
                    for ty in types {
                        if mimes.contains(ty) {
                            mime = ty.to_string();
                        }
                    }
                });

                let reader = match offer.receive(mime.clone()) {
                    Ok(reader) => reader,
                    Err(_) => return,
                };

                read_clipboard(reader, mime, metadata, window_id, &loop_handle);
            })
        });
    }
}

/// Handle writing to clipboard's pipe write end.
fn write_clipboard(
    writer: WritePipe,
    content: Rc<dyn AsRef<[u8]> + 'static>,
    loop_handle: &LoopHandle<'static, WinitState>,
) {
    let mut writer = unsafe {
        match raw_fd_into_non_blocking_file(writer.into_raw_fd()) {
            Ok(writer) => writer,
            Err(_) => return,
        }
    };

    let written = match writer.write((&*content).as_ref()) {
        Ok(n) if n == (&*content).as_ref().len() => return,
        Ok(n) => n,
        Err(err) if err.kind() == ErrorKind::WouldBlock => 0,
        Err(_) => return,
    };

    // We weren't able to write all content at once, so add pipe as a `calloop`'s event source
    // and continue writing when current content will be read.
    let mut written = Rc::new(written);
    let left_to_write = content.clone();
    let writer = Generic::new(writer, Interest::WRITE, Mode::Level);
    let _ = loop_handle.insert_source(writer, move |_, file, _| {
        let left_to_write = (&*left_to_write).as_ref();
        let (n, action) = match file.write(&left_to_write[*written..]) {
            Ok(n) if *written + n == left_to_write.len() => (n, PostAction::Remove),
            Ok(n) => (n, PostAction::Continue),
            Err(err) if err.kind() == ErrorKind::WouldBlock => (0, PostAction::Continue),
            Err(_) => (0, PostAction::Remove),
        };

        *Rc::get_mut(&mut written).unwrap() += n;
        Ok(action)
    });
}

/// Handle writing to clipboard's pipes read end.
fn read_clipboard(
    reader: ReadPipe,
    mime: String,
    metadata: Option<Arc<ClipboardMetadata>>,
    window_id: WindowId,
    loop_handle: &LoopHandle<'static, WinitState>,
) {
    let reader = unsafe {
        match raw_fd_into_non_blocking_file(reader.into_raw_fd()) {
            Ok(reader) => reader,
            Err(_) => return,
        }
    };
    let mut content = Vec::<u8>::with_capacity(u16::MAX as usize);
    let mut reader_buffer = [0; u16::MAX as usize];
    let reader = Generic::new(reader, Interest::READ, Mode::Level);
    let _ = loop_handle.insert_source(reader, move |_, file, winit_state| {
        let action = loop {
            match file.read(&mut reader_buffer) {
                Ok(0) => {
                    let event_sink = &mut winit_state.event_sink;
                    let data = std::mem::take(&mut content);
                    let content = ClipboardContent {
                        data,
                        mime: mime.clone(),
                        metadata: metadata.clone(),
                    };
                    let window_event = WindowEvent::ClipboardContent(content);
                    event_sink.push_window_event(window_event, window_id);
                    break PostAction::Remove;
                }
                Ok(n) => content.extend_from_slice(&reader_buffer[..n]),
                Err(err) if err.kind() == ErrorKind::WouldBlock => break PostAction::Continue,
                Err(_) => break PostAction::Remove,
            }
        };

        Ok(action)
    });
}

/// Create a `File` from `RawFd` marking it with `O_NONBLOCK`.
unsafe fn raw_fd_into_non_blocking_file(raw_fd: RawFd) -> Result<File> {
    let flags = libc::fcntl(raw_fd, libc::F_GETFD);
    if flags < 0 {
        return Err(Error::from_raw_os_error(flags));
    }
    let result = libc::fcntl(raw_fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    if result < 0 {
        return Err(Error::from_raw_os_error(result));
    }

    Ok(File::from_raw_fd(raw_fd))
}
