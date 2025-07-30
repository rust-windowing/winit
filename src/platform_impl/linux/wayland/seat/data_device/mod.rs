use sctk::data_device_manager::data_device::DataDeviceHandler;
use sctk::data_device_manager::data_offer::{DataOfferHandler, DragOffer};
use sctk::data_device_manager::data_source::DataSourceHandler;
use sctk::data_device_manager::WritePipe;
use wayland_client::protocol::wl_data_device::WlDataDevice;
use wayland_client::protocol::wl_data_device_manager::DndAction;
use wayland_client::protocol::wl_data_source::WlDataSource;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Proxy, QueueHandle};

use crate::event::WindowEvent;
use crate::platform_impl::wayland::state::WinitState;
use crate::platform_impl::wayland::types::dnd::DndOfferState;
use crate::platform_impl::wayland::{self};

const SUPPORTED_MIME_TYPES: &[&str] = &["text/uri-list"];

fn filter_mime(mime_types: &[String]) -> Option<String> {
    for mime in mime_types {
        if SUPPORTED_MIME_TYPES.contains(&mime.as_str()) {
            return Some(mime.clone());
        }
    }

    None
}

impl DataDeviceHandler for WinitState {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        wl_data_device: &WlDataDevice,
        _: f64,
        _: f64,
        _: &WlSurface,
    ) {
        let data_device = self.get_data_device(wl_data_device);

        if let Some(data_device) = data_device {
            if let Some(offer) = data_device.data().drag_offer() {
                if let Some(mime_type) = offer.with_mime_types(filter_mime) {
                    offer.accept_mime_type(offer.serial, Some(mime_type.clone()));
                    offer.set_actions(DndAction::Copy, DndAction::Copy);

                    if let Ok(read_pipe) = offer.receive(mime_type) {
                        let data_device_id = data_device.inner().id();
                        let surface = offer.surface;
                        let window_id = wayland::make_wid(&surface);

                        self.read_file_paths(read_pipe, move |state, path| {
                            state.dnd_offers.insert(data_device_id.clone(), DndOfferState {
                                surface: surface.clone(),
                                path: path.clone(),
                            });

                            state
                                .events_sink
                                .push_window_event(WindowEvent::HoveredFile(path), window_id);
                        });
                    }
                }
            }
        }
    }

    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, wl_data_device: &WlDataDevice) {
        let data_device = self.get_data_device(wl_data_device);

        if let Some(data_device) = data_device {
            if let Some(dnd_offer) = self.dnd_offers.remove(&data_device.inner().id()) {
                let window_id = wayland::make_wid(&dnd_offer.surface);
                self.events_sink.push_window_event(WindowEvent::HoveredFileCancelled, window_id);
            }
        }
    }

    fn motion(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        wl_data_device: &WlDataDevice,
        _: f64,
        _: f64,
    ) {
        let data_device = self.get_data_device(wl_data_device);

        if let Some(data_device) = data_device {
            if let Some(offer) = data_device.data().drag_offer() {
                let window_id = wayland::make_wid(&offer.surface);

                if let Some(dnd_offer) = self.dnd_offers.get(&data_device.inner().id()) {
                    self.events_sink.push_window_event(
                        WindowEvent::HoveredFile(dnd_offer.path.to_path_buf()),
                        window_id,
                    );
                }
            }
        }
    }

    fn selection(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}

    fn drop_performed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        wl_data_device: &WlDataDevice,
    ) {
        let data_device = self.get_data_device(wl_data_device);

        if let Some(data_device) = data_device {
            if let Some(offer) = data_device.data().drag_offer() {
                let window_id = wayland::make_wid(&offer.surface);

                if let Some(dnd_offer) = self.dnd_offers.remove(&data_device.inner().id()) {
                    self.events_sink
                        .push_window_event(WindowEvent::DroppedFile(dnd_offer.path), window_id);

                    offer.finish();
                    offer.destroy();
                }
            }
        }
    }
}

impl DataSourceHandler for WinitState {
    fn accept_mime(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlDataSource,
        _: Option<String>,
    ) {
    }

    fn send_request(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlDataSource,
        _: String,
        _: WritePipe,
    ) {
    }

    fn cancelled(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}

    fn dnd_dropped(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}

    fn dnd_finished(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}

    fn action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: DndAction) {}
}

impl DataOfferHandler for WinitState {
    fn source_actions(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        offer: &mut DragOffer,
        _: DndAction,
    ) {
        offer.set_actions(DndAction::Copy, DndAction::Copy);
    }

    fn selected_action(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &mut DragOffer,
        _: DndAction,
    ) {
    }
}

sctk::delegate_data_device!(WinitState);
