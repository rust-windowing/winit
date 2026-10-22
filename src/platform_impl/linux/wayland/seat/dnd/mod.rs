//! Native Wayland file drops. Source-controlled metadata is bounded and read off the dispatch path.
#[cfg(feature = "wayland-portal")]
mod portal;
mod uris;

use crate::dpi::LogicalPosition;
use crate::event::WindowEvent;
use crate::platform_impl::wayland::state::WinitState;
use crate::platform_impl::wayland::{self};
use ahash::AHashMap;
use sctk::reexports::calloop::timer::{TimeoutAction, Timer};
use sctk::reexports::calloop::{PostAction, RegistrationToken};
use sctk::reexports::client::backend::ObjectId;
use sctk::reexports::client::protocol::wl_data_device::{self, WlDataDevice};
use sctk::reexports::client::protocol::wl_data_device_manager::{DndAction, WlDataDeviceManager};
use sctk::reexports::client::protocol::wl_data_offer::{self, WlDataOffer};
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::{event_created_child, Connection, Dispatch, Proxy, QueueHandle};
use std::io::Read;
use std::sync::Mutex;
use std::time::Duration;

const URI_LIST: &str = "text/uri-list";
const PORTAL: &str = "application/vnd.portal.filetransfer";
const MAX_PENDING: usize = 8;

#[derive(Default)]
pub(crate) struct DndState {
    next_id: u64,
    active: AHashMap<ObjectId, DragOffer>,
    pending: AHashMap<u64, Pending>,
}
struct Pending {
    offer: DragOffer,
    pipe: Option<RegistrationToken>,
    timer: RegistrationToken,
}

pub(crate) struct DeviceData;
#[derive(Default)]
pub(crate) struct OfferData(Mutex<OfferMetadata>);
struct OfferMetadata {
    mime: Option<&'static str>,
    selected: DndAction,
}
impl Default for OfferMetadata {
    fn default() -> Self {
        Self { mime: None, selected: DndAction::empty() }
    }
}
#[derive(Clone)]
struct DragOffer {
    raw: WlDataOffer,
    serial: u32,
    surface: WlSurface,
    x: f64,
    y: f64,
}
impl DragOffer {
    fn inner(&self) -> &WlDataOffer {
        &self.raw
    }
    fn destroy(&self) {
        self.raw.destroy();
    }
    fn finish(&self) {
        if self.raw.version() >= 3 {
            self.raw.finish();
        }
    }
    fn metadata(&self) -> Option<std::sync::MutexGuard<'_, OfferMetadata>> {
        self.raw.data::<OfferData>()?.0.lock().ok()
    }
    fn selected_action(&self) -> DndAction {
        self.metadata().map_or(DndAction::empty(), |m| m.selected)
    }
    fn receive(&self, mime: String) -> std::io::Result<sctk::data_device_manager::ReadPipe> {
        sctk::data_device_manager::data_offer::receive(&self.raw, mime)
    }
    fn accept_mime_type(&self, serial: u32, mime: Option<String>) {
        self.raw.accept(serial, mime);
    }
    fn set_actions(&self, actions: DndAction, preferred: DndAction) {
        self.raw.set_actions(actions, preferred);
    }
}
fn mime(offer: &DragOffer) -> Option<&'static str> {
    offer.metadata()?.mime
}
fn accepts(offer: &DragOffer) -> bool {
    mime(offer).is_some()
}

impl WinitState {
    fn drag_position(&mut self, surface: &WlSurface, x: f64, y: f64) {
        let wid = wayland::make_wid(surface);
        let scale =
            self.windows.borrow().get(&wid).and_then(|w| w.lock().ok().map(|w| w.scale_factor()));
        if let Some(scale) = scale {
            self.events_sink.push_window_event(
                WindowEvent::CursorMoved {
                    device_id: crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(
                        wayland::DeviceId,
                    )),
                    position: LogicalPosition::new(x, y).to_physical(scale),
                },
                wid,
            );
            self.dispatched_events = true;
        }
    }

    pub(crate) fn cancel_window_drops(&mut self, window: wayland::WindowId) {
        let ids: Vec<_> = self
            .dnd
            .pending
            .iter()
            .filter_map(|(id, p)| (wayland::make_wid(&p.offer.surface) == window).then_some(*id))
            .collect();
        for id in ids {
            self.complete_drop(id, None);
        }
        self.dnd.active.retain(|_, offer| {
            if wayland::make_wid(&offer.surface) == window {
                offer.destroy();
                false
            } else {
                true
            }
        });
    }

    fn complete_drop(&mut self, id: u64, paths: Option<Vec<std::path::PathBuf>>) {
        let Some(pending) = self.dnd.pending.remove(&id) else { return };
        if let Some(pipe) = pending.pipe {
            self.loop_handle.remove(pipe);
        }
        self.loop_handle.remove(pending.timer);
        let wid = wayland::make_wid(&pending.offer.surface);
        if let Some(paths) = paths {
            if self.windows.borrow().contains_key(&wid) {
                self.drag_position(&pending.offer.surface, pending.offer.x, pending.offer.y);
                for path in paths {
                    self.events_sink.push_window_event(WindowEvent::DroppedFile(path), wid);
                }
                self.dispatched_events = true;
            }
            pending.offer.finish();
        }
        pending.offer.destroy();
    }
}

impl WinitState {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        device: &WlDataDevice,
        x: f64,
        y: f64,
        surface: &WlSurface,
    ) {
        if let Some(offer) = self.dnd.active.get(&device.id()).cloned() {
            let accepted =
                accepts(&offer) && self.windows.borrow().contains_key(&wayland::make_wid(surface));
            offer.accept_mime_type(
                offer.serial,
                mime(&offer).filter(|_| accepted).map(str::to_owned),
            );
            if offer.inner().version() >= 3 {
                offer.set_actions(
                    if accepted { DndAction::Copy } else { DndAction::empty() },
                    if accepted { DndAction::Copy } else { DndAction::empty() },
                );
            }
            if accepted {
                self.drag_position(surface, x, y);
            }
        }
    }
    fn motion(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        device: &WlDataDevice,
        x: f64,
        y: f64,
    ) {
        if let Some(offer) = self.dnd.active.get(&device.id()).cloned().filter(accepts) {
            self.drag_position(&offer.surface, x, y);
        }
    }
    pub(crate) fn cancel_drag(&mut self, device: &WlDataDevice) {
        if let Some(offer) = self.dnd.active.remove(&device.id()) {
            let wid = wayland::make_wid(&offer.surface);
            offer.destroy();
            if self.windows.borrow().contains_key(&wid) {
                self.events_sink.push_window_event(WindowEvent::HoveredFileCancelled, wid);
                self.dispatched_events = true;
            }
        }
    }
    fn drop_performed(&mut self, _: &Connection, _: &QueueHandle<Self>, device: &WlDataDevice) {
        let Some(offer) = self.dnd.active.remove(&device.id()) else { return };
        if !self.windows.borrow().contains_key(&wayland::make_wid(&offer.surface))
            || !accepts(&offer)
            || self.dnd.pending.len() >= MAX_PENDING
            || (offer.inner().version() >= 3 && offer.selected_action() != DndAction::Copy)
        {
            offer.destroy();
            return;
        }
        let Some(chosen_mime) = mime(&offer) else {
            offer.destroy();
            return;
        };
        let Ok(pipe) = offer.receive(chosen_mime.to_owned()) else {
            offer.destroy();
            return;
        };
        self.dnd.next_id = self.dnd.next_id.wrapping_add(1);
        let id = self.dnd.next_id;
        let mut bytes = Vec::new();
        let Ok(pipe_token) = self.loop_handle.insert_source(pipe, move |_, file, state| {
            let mut buffer = [0; 8192];
            let mut reader: &std::fs::File = file;
            match reader.read(&mut buffer) {
                Ok(0) => {
                    if chosen_mime == URI_LIST {
                        state.complete_drop(id, uris::file_paths(&bytes).ok());
                    } else {
                        #[cfg(feature = "wayland-portal")]
                        state.resolve_portal_drop(id, &bytes);
                    }
                    PostAction::Remove
                },
                Ok(n) if bytes.len() + n <= uris::MAX_BYTES => {
                    bytes.extend_from_slice(&buffer[..n]);
                    PostAction::Continue
                },
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) =>
                {
                    PostAction::Continue
                },
                _ => {
                    state.complete_drop(id, None);
                    PostAction::Remove
                },
            }
        }) else {
            offer.destroy();
            return;
        };
        let Ok(timer) = self.loop_handle.insert_source(
            Timer::from_duration(Duration::from_secs(10)),
            move |_, _, state| {
                state.complete_drop(id, None);
                TimeoutAction::Drop
            },
        ) else {
            self.loop_handle.remove(pipe_token);
            offer.destroy();
            return;
        };
        self.dnd.pending.insert(id, Pending { offer, pipe: Some(pipe_token), timer });
    }
}

// Keep dropped offers separate from hover offers: a second drag must not destroy an
// earlier accepted transfer whose pipe or portal reply is still pending.
impl Dispatch<WlDataDevice, DeviceData> for WinitState {
    event_created_child!(WinitState, WlDataDevice, [0 => (WlDataOffer, OfferData::default())]);
    fn event(
        state: &mut Self,
        device: &WlDataDevice,
        event: wl_data_device::Event,
        _: &DeviceData,
        conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_device::Event::DataOffer { .. } => {},
            wl_data_device::Event::Enter { serial, surface, x, y, id } => {
                state.cancel_drag(device);
                if let Some(raw) = id {
                    state.dnd.active.insert(device.id(), DragOffer {
                        raw,
                        serial,
                        surface: surface.clone(),
                        x,
                        y,
                    });
                    state.enter(conn, qh, device, x, y, &surface);
                }
            },
            wl_data_device::Event::Motion { x, y, .. } => {
                if let Some(offer) = state.dnd.active.get_mut(&device.id()) {
                    offer.x = x;
                    offer.y = y;
                }
                state.motion(conn, qh, device, x, y);
            },
            wl_data_device::Event::Drop => state.drop_performed(conn, qh, device),
            wl_data_device::Event::Leave => state.cancel_drag(device),
            // This second device never claims clipboard selections; destroy unsolicited offers.
            wl_data_device::Event::Selection { id: Some(offer) } => offer.destroy(),
            _ => {},
        }
    }
}
impl Dispatch<WlDataOffer, OfferData> for WinitState {
    fn event(
        _: &mut Self,
        _: &WlDataOffer,
        event: wl_data_offer::Event,
        data: &OfferData,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Ok(mut info) = data.0.lock() else { return };
        match event {
            wl_data_offer::Event::Offer { mime_type } => {
                if mime_type == PORTAL && cfg!(feature = "wayland-portal") {
                    info.mime = Some(PORTAL);
                } else if mime_type == URI_LIST && info.mime.is_none() {
                    info.mime = Some(URI_LIST);
                }
            },
            wl_data_offer::Event::Action { dnd_action } => {
                info.selected = dnd_action.into_result().unwrap_or(DndAction::empty())
            },
            _ => {},
        }
    }
}
sctk::reexports::client::delegate_dispatch!(WinitState: [WlDataDeviceManager: sctk::globals::GlobalData] => sctk::data_device_manager::DataDeviceManagerState);
