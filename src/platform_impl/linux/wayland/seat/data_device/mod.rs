use std::io::{BufRead, BufReader};
use std::os::fd::AsFd;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use calloop::{LoopHandle, PostAction, RegistrationToken};
use rustix::pipe::{pipe_with, PipeFlags};
use sctk::data_device_manager::data_source::DataSourceData;
use sctk::data_device_manager::ReadPipe;
use sctk::globals::GlobalData;
use sctk::reexports::client::globals::{BindError, GlobalList};
use sctk::reexports::client::protocol::wl_data_device::{Event as WlDataDeviceEvent, WlDataDevice};
use sctk::reexports::client::protocol::wl_data_device_manager::{DndAction, WlDataDeviceManager};
use sctk::reexports::client::protocol::wl_data_offer::{Event as WlDataOfferEvent, WlDataOffer};
use sctk::reexports::client::protocol::wl_data_source::WlDataSource;
use sctk::reexports::client::protocol::wl_seat::WlSeat;
use sctk::reexports::client::{
    delegate_dispatch, event_created_child, Connection, Dispatch, Proxy, QueueHandle,
};
use tracing::warn;

use crate::event::WindowEvent;
use crate::platform_impl::wayland;
use crate::platform_impl::wayland::state::WinitState;
use crate::window::WindowId;

pub struct DataDeviceManager {
    manager: WlDataDeviceManager,
}

impl DataDeviceManager {
    pub fn new(
        globals: &GlobalList,
        queue_handle: &QueueHandle<WinitState>,
    ) -> Result<Self, BindError> {
        let manager = globals.bind(queue_handle, 1..=3, GlobalData)?;
        Ok(Self { manager })
    }

    pub fn init_seat_data_device(&self, seat: &WlSeat, queue_handle: &QueueHandle<WinitState>) {
        let data = DataDeviceData { seat: seat.clone() };
        self.manager.get_data_device(seat, queue_handle, data);
    }
}

impl Dispatch<WlDataDeviceManager, GlobalData, WinitState> for DataDeviceState {
    fn event(
        _state: &mut WinitState,
        _proxy: &WlDataDeviceManager,
        _event: <WlDataDeviceManager as Proxy>::Event,
        _data: &GlobalData,
        _conn: &Connection,
        _qhandle: &QueueHandle<WinitState>,
    ) {
    }
}

struct DataDeviceData {
    seat: WlSeat,
}

impl Dispatch<WlDataDevice, DataDeviceData, WinitState> for DataDeviceState {
    event_created_child!(WinitState, WlDataDevice, [
        0 => (WlDataOffer, Default::default())
    ]);

    fn event(
        state: &mut WinitState,
        wl_data_device: &WlDataDevice,
        event: <WlDataDevice as Proxy>::Event,
        data: &DataDeviceData,
        _conn: &Connection,
        _qhandle: &QueueHandle<WinitState>,
    ) {
        let seat_state = match state.seats.get_mut(&data.seat.id()) {
            Some(seat_state) => seat_state,
            None => {
                warn!("Received data device event {event:?} without seat");
                return;
            },
        };

        let data_device_state = match seat_state.data_device_state.as_mut() {
            Some(data_device_state) => data_device_state,
            None => {
                warn!("Received data device event {event:?} without data device state");
                return;
            },
        };

        match event {
            WlDataDeviceEvent::Enter { serial, surface, x: _, y: _, id } => {
                let Some(wl_data_offer) = id else {
                    warn!("Received data device enter event without id");
                    return;
                };

                let Some(data_offer) = wl_data_offer.data::<DataOfferData>() else {
                    warn!("Received data device enter event without data offer");
                    return;
                };

                let Some(mime_type) = (match data_offer.mime_types.lock() {
                    Ok(mime_types) => {
                        mime_types.iter().find(|&mime| mime == FILE_TRANSFER_MIME_TYPE).cloned()
                    },
                    Err(e) => {
                        warn!("Failed to lock MIME type vector: {e}");
                        None
                    },
                }) else {
                    warn!("Data deviced entered with no valid MIME type");
                    return;
                };

                wl_data_offer.set_actions(DndAction::Copy, DndAction::Copy);
                wl_data_offer.accept(serial, Some(mime_type.clone()));

                let (read_fd, write_fd) = match pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK)
                {
                    Ok((read_fd, write_fd)) => (read_fd, write_fd),
                    Err(e) => {
                        warn!("Failed to create pipe to read data offer from: {e}");
                        return;
                    },
                };

                wl_data_offer.receive(mime_type, write_fd.as_fd());
                let read_pipe = ReadPipe::from(read_fd);

                if let Some(token) = data_device_state.read_token.take() {
                    warn!("Cancelling previous data device enter read");
                    data_device_state.loop_handle.remove(token);
                }

                let window_id = wayland::make_wid(&surface);
                data_device_state.hovered_window = Some(window_id);
                data_device_state.offer = Some(wl_data_offer);
                data_device_state.read_buffer.clear();

                let wl_data_device = wl_data_device.clone();
                data_device_state.read_token = data_device_state
                    .loop_handle
                    .insert_source(read_pipe, move |_, f, state| {
                        state.dispatched_events = true;
                        let data = wl_data_device.data::<DataDeviceData>().unwrap();

                        let seat_state = match state.seats.get_mut(&data.seat.id()) {
                            Some(seat_state) => seat_state,
                            None => return PostAction::Remove,
                        };

                        let data_device_state = match seat_state.data_device_state.as_mut() {
                            Some(data_device_state) => data_device_state,
                            None => return PostAction::Remove,
                        };

                        let f: &mut std::fs::File = unsafe { f.get_mut() };
                        let mut reader = BufReader::new(f);

                        let consumed = match reader.fill_buf() {
                            Ok(buf) => {
                                if buf.is_empty() {
                                    if let Some(hovered) = data_device_state.hovered_window {
                                        data_device_state.with_file_paths(|paths| {
                                            for path in paths {
                                                state.events_sink.push_window_event(
                                                    WindowEvent::HoveredFile(path),
                                                    hovered,
                                                );
                                            }
                                        });
                                    } else {
                                        warn!("No window specified to push HoveredFile to");
                                    }
                                    return PostAction::Remove;
                                } else {
                                    data_device_state.read_buffer.extend_from_slice(buf);
                                }
                                buf.len()
                            },
                            Err(e)
                                if matches!(
                                    e.kind(),
                                    std::io::ErrorKind::Interrupted
                                        | std::io::ErrorKind::WouldBlock
                                ) =>
                            {
                                return PostAction::Continue;
                            },
                            Err(_) => {
                                return PostAction::Remove;
                            },
                        };
                        reader.consume(consumed);

                        PostAction::Continue
                    })
                    .ok();
            },
            WlDataDeviceEvent::Motion { time: _, x: _, y: _ } => {},
            WlDataDeviceEvent::Drop => {
                if let Some(hovered_window) = data_device_state.hovered_window {
                    data_device_state.with_file_paths(|paths| {
                        for path in paths {
                            state
                                .events_sink
                                .push_window_event(WindowEvent::DroppedFile(path), hovered_window);
                        }
                    });
                    data_device_state.finish_offer();
                }
            },
            WlDataDeviceEvent::Leave => {
                if let Some(hovered_window) = data_device_state.hovered_window {
                    state
                        .events_sink
                        .push_window_event(WindowEvent::HoveredFileCancelled, hovered_window);
                    data_device_state.finish_offer();
                }
            },
            WlDataDeviceEvent::DataOffer { id: _ } => {},
            WlDataDeviceEvent::Selection { id: _ } => {},
            _ => unreachable!(),
        }
    }
}

#[derive(Default)]
struct DataOfferData {
    pub mime_types: Arc<Mutex<Vec<String>>>,
}

impl Dispatch<WlDataOffer, DataOfferData, WinitState> for DataDeviceState {
    fn event(
        _state: &mut WinitState,
        _wl_data_offer: &WlDataOffer,
        event: <WlDataOffer as Proxy>::Event,
        data: &DataOfferData,
        _conn: &Connection,
        _qhandle: &QueueHandle<WinitState>,
    ) {
        match event {
            WlDataOfferEvent::SourceActions { source_actions: _ } => {},
            WlDataOfferEvent::Action { dnd_action: _ } => {},
            WlDataOfferEvent::Offer { mime_type } => match data.mime_types.lock() {
                Ok(ref mut mime_types) => {
                    mime_types.push(mime_type);
                },
                Err(e) => {
                    warn!("Failed to lock data offer mime types: {e}");
                },
            },
            _ => unreachable!(),
        }
    }
}

impl Dispatch<WlDataSource, DataSourceData, WinitState> for DataDeviceState {
    fn event(
        _state: &mut WinitState,
        _proxy: &WlDataSource,
        _event: <WlDataSource as Proxy>::Event,
        _data: &DataSourceData,
        _conn: &Connection,
        _qhandle: &QueueHandle<WinitState>,
    ) {
    }
}

#[derive(Debug)]
pub struct DataDeviceState {
    pub loop_handle: LoopHandle<'static, WinitState>,
    pub hovered_window: Option<WindowId>,
    pub offer: Option<WlDataOffer>,
    pub read_token: Option<RegistrationToken>,
    pub read_buffer: Vec<u8>,
}

impl DataDeviceState {
    pub fn new(loop_handle: LoopHandle<'static, WinitState>) -> Self {
        Self {
            loop_handle,
            hovered_window: None,
            offer: None,
            read_token: None,
            read_buffer: Vec::new(),
        }
    }

    fn with_file_paths(&mut self, mut callback: impl FnMut(Vec<PathBuf>)) {
        let mut paths = Vec::new();
        if let Ok(uri_list) = String::from_utf8(self.read_buffer.clone()) {
            for line in uri_list.lines() {
                if line.starts_with("#") {
                    continue;
                }

                if let Some(file) = line.strip_prefix("file://") {
                    paths.push(PathBuf::from(file));
                } else {
                    warn!("Non-comment line in URI list missing prefix: '{line}'");
                }
            }
        } else {
            warn!("Failed to parse URI list from hovered file");
        }
        callback(paths);
    }

    fn finish_offer(&mut self) {
        self.hovered_window = None;
        if let Some(offer) = self.offer.take() {
            offer.finish();
            offer.destroy();
        }
        if let Some(token) = self.read_token.take() {
            self.loop_handle.remove(token);
        }
    }
}

const FILE_TRANSFER_MIME_TYPE: &str = "text/uri-list";

delegate_dispatch!(WinitState: [WlDataDeviceManager: GlobalData] => DataDeviceState);
delegate_dispatch!(WinitState: [WlDataDevice: DataDeviceData] => DataDeviceState);
delegate_dispatch!(WinitState: [WlDataOffer: DataOfferData] => DataDeviceState);
delegate_dispatch!(WinitState: [WlDataSource: DataSourceData] => DataDeviceState);
