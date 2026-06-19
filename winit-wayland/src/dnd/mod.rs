//! Types related to drag-and-drop and data transfer on Wayland.

#![warn(missing_docs)]

mod send_data;

use std::ffi::OsString;
use std::fmt;
use std::io::{self, BufRead, Cursor, ErrorKind};
use std::ops::{BitOr, Deref};
use std::os::unix::ffi::OsStringExt;
use std::sync::Arc;

use calloop::PostAction;
use dpi::{LogicalPosition, PhysicalPosition};
use sctk::data_device_manager::WritePipe;
use sctk::data_device_manager::data_device::{DataDeviceData, DataDeviceHandler};
use sctk::data_device_manager::data_offer::{DataOfferHandler, DragOffer};
use sctk::data_device_manager::data_source::{DataSourceHandler, DragSource as SctkDragSource};
use sctk::reexports::client::backend::ObjectId;
use wayland_client::protocol::wl_data_device::WlDataDevice;
use wayland_client::protocol::wl_data_device_manager::DndAction as WlDndAction;
use wayland_client::protocol::wl_data_offer::WlDataOffer;
use wayland_client::protocol::wl_data_source::WlDataSource;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Proxy, QueueHandle};
use winit_core::data_transfer::{
    DataTransfer, DataTransferId, DataTransferSend, SendData, TransferType, TypeHint, TypedData,
};
use winit_core::event::WindowEvent;
use winit_core::event_loop::DndAction;
use winit_core::window::WindowId;

use crate::dnd::send_data::SendDataEncoder;
use crate::make_data_transfer_id;
use crate::state::WinitState;

impl DataSourceHandler for WinitState {
    fn accept_mime(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlDataSource,
        _: Option<String>,
    ) {
        // This method isn't a necessary part of the protocol, it's a holdover from the first
        // version of DnD in Wayland and now just serves as a hint.
    }

    fn send_request(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlDataSource,
        mime: String,
        fd: WritePipe,
    ) {
        let Some(data) = self.dnd_state.send_drag_data_mut() else {
            // TODO: Is there a way to explicitly express that the data was not sent?
            return;
        };

        let mime = MimeType::parse(mime);

        let Some(send_data) = data.data_for_type(&mime) else {
            return;
        };

        let mut encoder = match send_data {
            SendData::Uris(os_strings) => SendDataEncoder::Uris(os_strings.into()),
            SendData::String(str) => match mime.parse_charset().unwrap_or(mime.default_charset()) {
                Charset::Utf8 => SendDataEncoder::Bytes(Cursor::new(str.into_bytes())),
                Charset::Utf16 => {
                    // TODO: It wouldn't be too difficult to make this lazy
                    let utf16_binary = str
                        .encode_utf16()
                        // I can't find any documentation on whether Wayland UTF-16 is required to
                        // be little-, big-, or native-endian, but little-endian seems to work so
                        // we'll make the assumption that it's consistent cross-platform for now.
                        .flat_map(|uint16| uint16.to_le_bytes())
                        .collect::<Vec<_>>();
                    SendDataEncoder::Bytes(Cursor::new(utf16_binary))
                },
            },
            SendData::Bytes(binary) => SendDataEncoder::Bytes(Cursor::new(binary)),
        };

        let _ = self.loop_handle.insert_source(fd, move |_, file, _| {
            // Safety: We only mutate `file` in-place and do not replace and drop it.
            let file = unsafe { file.get_mut() };
            loop {
                match std::io::copy(&mut encoder, file) {
                    Ok(0) => {
                        break PostAction::Remove;
                    },
                    Ok(_) => {},
                    Err(e) if e.kind() == ErrorKind::WouldBlock => {
                        break PostAction::Continue;
                    },
                    Err(_) => {
                        break PostAction::Remove;
                    },
                }
            }
        });
    }

    // TODO: Send `DragCancel` event.
    fn cancelled(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {
        self.dnd_state.clear_send_drag();
    }

    fn dnd_dropped(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        source: &wayland_client::protocol::wl_data_source::WlDataSource,
    ) {
        let _ = source;
        let _ = qh;
        let _ = conn;

        let Some(current_drag) = self.dnd_state.send_drag() else {
            return;
        };

        let window_id = current_drag.window_id;
        let id = current_drag.data_transfer_id;
        let selected_action = current_drag.selected_action;

        self.events_sink.push_window_event(
            WindowEvent::OutgoingDragEnded { id, action: dnd_action_wl_to_winit(selected_action) },
            window_id,
        );
    }

    fn dnd_finished(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        source: &wayland_client::protocol::wl_data_source::WlDataSource,
    ) {
        let _ = source;
        let _ = qh;
        let _ = conn;
        self.dnd_state.clear_send_drag();
    }

    fn action(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        source: &WlDataSource,
        action: WlDndAction,
    ) {
        let _ = action;
        let _ = source;
        let _ = qh;
        let _ = conn;

        self.dnd_state.set_target_drag_action(action);
    }
}

#[derive(Default, Debug, PartialEq, Eq, Clone, Hash)]
enum Charset {
    #[default]
    Utf8,
    Utf16,
}

/// MIME type as string, with an optional hint detected from the MIME type.
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct MimeType {
    mime: Arc<str>,
    hint: Option<TypeHint>,
}

// MIME types
// Files
const TEXT_URI_LIST: &str = "text/uri-list";
// Plaintext
const TEXT_PLAIN: &str = "text/plain";
const TEXT_PLAIN_CHARSET_UTF8: &str = "text/plain; charset=utf-8";
// HTML
const TEXT_HTML: &str = "text/html";
const TEXT_HTML_CHARSET_UTF8: &str = "text/html; charset=utf-8";
// RTF
const APPLICATION_RTF: &str = "application/rtf";
// Audio
const AUDIO_AAC: &str = "audio/aac";
const AUDIO_AIFF: &str = "audio/aiff";
const AUDIO_FLAC: &str = "audio/flac";
const AUDIO_WAV: &str = "audio/wav";
const AUDIO_WAVE: &str = "audio/wave";
const AUDIO_X_WAV: &str = "audio/x-wav";
const AUDIO_VND_WAV: &str = "audio/vnd.wav";
const AUDIO_VND_WAVE: &str = "audio/vnd.wave";
const AUDIO_MPEG: &str = "audio/mpeg";
const AUDIO_OGG: &str = "audio/ogg";
// Image
const IMAGE_BMP: &str = "image/bmp";
const IMAGE_GIF: &str = "image/gif";
const IMAGE_JPEG: &str = "image/jpeg";
const IMAGE_PJPEG: &str = "image/pjpeg";
const IMAGE_PNG: &str = "image/png";
const IMAGE_SVG: &str = "image/svg+xml";
const IMAGE_TIFF: &str = "image/tiff";
const IMAGE_WEBP: &str = "image/webp";
const IMAGE_X_ICON: &str = "image/x-icon";
const IMAGE_RAW: &str = "image/x-panasonic-raw";

impl MimeType {
    const MIME_HINT_MAP: &[(&str, TypeHint)] = &[
        // Files
        (TEXT_URI_LIST, TypeHint::UriList),
        // Plaintext
        (TEXT_PLAIN, TypeHint::Plaintext),
        (TEXT_PLAIN_CHARSET_UTF8, TypeHint::Plaintext),
        // HTML
        (TEXT_HTML, TypeHint::Html),
        (TEXT_HTML_CHARSET_UTF8, TypeHint::Html),
        // RTF
        (APPLICATION_RTF, TypeHint::Rtf),
        // Audio
        (AUDIO_AAC, TypeHint::Audio { extension_hint: Some("aac") }),
        (AUDIO_AIFF, TypeHint::Audio { extension_hint: Some("aif") }),
        (AUDIO_FLAC, TypeHint::Audio { extension_hint: Some("flac") }),
        (AUDIO_VND_WAV, TypeHint::Audio { extension_hint: Some("wav") }),
        (AUDIO_VND_WAVE, TypeHint::Audio { extension_hint: Some("wav") }),
        (AUDIO_WAV, TypeHint::Audio { extension_hint: Some("wav") }),
        (AUDIO_WAVE, TypeHint::Audio { extension_hint: Some("wav") }),
        (AUDIO_X_WAV, TypeHint::Audio { extension_hint: Some("wav") }),
        (AUDIO_OGG, TypeHint::Audio { extension_hint: Some("ogg") }),
        (AUDIO_MPEG, TypeHint::Audio { extension_hint: Some("mp3") }),
        // Image
        (IMAGE_BMP, TypeHint::Image { extension_hint: Some("bmp") }),
        (IMAGE_GIF, TypeHint::Image { extension_hint: Some("gif") }),
        (IMAGE_JPEG, TypeHint::Image { extension_hint: Some("jpg") }),
        (IMAGE_PJPEG, TypeHint::Image { extension_hint: Some("jpg") }),
        (IMAGE_PNG, TypeHint::Image { extension_hint: Some("png") }),
        (IMAGE_RAW, TypeHint::Image { extension_hint: Some("raw") }),
        (IMAGE_SVG, TypeHint::Image { extension_hint: Some("svg") }),
        (IMAGE_TIFF, TypeHint::Image { extension_hint: Some("tiff") }),
        (IMAGE_WEBP, TypeHint::Image { extension_hint: Some("webp") }),
        (IMAGE_X_ICON, TypeHint::Image { extension_hint: Some("ico") }),
    ];

    // Returns an iterator so that things like the multiple charsets for plaintext/HTML
    // and the multiple ways of expressing .wav work correctly.
    pub(crate) fn from_dyn(type_: &dyn TransferType) -> impl Iterator<Item = Self> {
        let downcast = type_.cast_ref::<Self>().cloned();
        let downcast_failed = downcast.is_none();
        // This filter is a bit hacky, but it's the only way to ensure that we always
        // return the same type.
        let from_hint = Some(()).filter(|_| downcast_failed).into_iter().flat_map(move |()| {
            Self::MIME_HINT_MAP
                .iter()
                .filter(move |(_, haystack)| TransferType::matches(haystack, type_))
                .map(move |(mime, _)| Self { mime: mime.to_string().into(), hint: type_.hint() })
        });

        downcast.into_iter().chain(from_hint)
    }

    // TODO: We should properly parse MIME types using `mime` or a similar crate.
    fn parse_charset(&self) -> Option<Charset> {
        let (_essence, options) = self.mime.split_once(';')?;

        let (_, charset) = options.split_once("charset=")?;

        if charset.starts_with("utf-8") {
            Some(Charset::Utf8)
        } else if charset.starts_with("utf-16") {
            Some(Charset::Utf16)
        } else {
            None
        }
    }

    fn default_charset(&self) -> Charset {
        match self.hint {
            Some(TypeHint::Html) => Charset::Utf16,
            _ => Charset::Utf8,
        }
    }

    fn parse(mime: String) -> Self {
        let hint = Self::MIME_HINT_MAP
            .iter()
            .find_map(|(haystack, hint)| (*haystack == &*mime).then_some(*hint))
            .or_else(|| {
                if mime.starts_with("image/") {
                    Some(TypeHint::Image { extension_hint: None })
                } else if mime.starts_with("audio/") {
                    Some(TypeHint::Audio { extension_hint: None })
                } else {
                    None
                }
            });

        Self { mime: mime.into(), hint }
    }
}

impl fmt::Display for MimeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.mime.fmt(f)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct UnknownTypeHint(pub TypeHint);

impl fmt::Display for UnknownTypeHint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unknown type hint: {:?}", self.0)
    }
}

impl TryFrom<TypeHint> for MimeType {
    type Error = UnknownTypeHint;

    fn try_from(hint: TypeHint) -> Result<Self, Self::Error> {
        let mime = Self::MIME_HINT_MAP
            .iter()
            .find_map(|(mime, haystack)| (*haystack == hint).then_some(*mime))
            .ok_or(UnknownTypeHint(hint))?;

        Ok(Self { mime: mime.to_owned().into(), hint: Some(hint) })
    }
}

impl TransferType for MimeType {
    fn hint(&self) -> Option<TypeHint> {
        self.hint
    }

    fn matches(&self, other: &dyn TransferType) -> bool {
        if let Some(other_mime) = other.cast_ref::<Self>() {
            *self == *other_mime
        } else {
            // If either hint is `None`, return false
            self.hint().is_some_and(|hint| other.hint() == Some(hint))
        }
    }
}

type BytesResult = Result<Vec<u8>, Arc<io::Error>>;

/// Typed data transfer from another application.
#[derive(Debug)]
pub struct MimeData {
    mime_type: MimeType,
    result: BytesResult,
}

impl MimeData {
    pub(crate) fn new(mime_type: MimeType, result: BytesResult) -> Self {
        Self { mime_type, result }
    }

    fn data(&self) -> io::Result<&[u8]> {
        fn arc_to_io_error(arc: Arc<io::Error>) -> io::Error {
            io::Error::new(arc.kind(), arc)
        }

        self.result.as_deref().map_err(|e| arc_to_io_error(e.clone()))
    }
}

impl TypedData for MimeData {
    fn type_(&self) -> &dyn TransferType {
        &self.mime_type
    }

    fn try_read(&self) -> Option<Box<dyn io::BufRead>> {
        let data = self.data().ok()?.to_owned();

        Some(Box::new(io::Cursor::new(data)))
    }

    fn try_as_bytes(&self) -> io::Result<Vec<u8>> {
        self.data().map(ToOwned::to_owned)
    }

    fn try_as_uris(&self) -> io::Result<Vec<OsString>> {
        let data = self.data()?;

        Cursor::new(&data)
            .lines()
            .filter(|result| match result {
                Ok(s) => !s.starts_with('#'),
                // We want to maintain errors, so the final `collect` returns an error too
                Err(_) => true,
            })
            .map(|res| {
                Ok(OsString::from_vec(percent_encoding::percent_decode_str(&res?).collect()))
            })
            .collect()
    }

    fn try_as_string(&self) -> io::Result<String> {
        let charset = self.mime_type.parse_charset().unwrap_or(self.mime_type.default_charset());

        let data = self.data()?;

        match charset {
            Charset::Utf8 => String::from_utf8(data.to_vec())
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err)),
            Charset::Utf16 => {
                // TODO: `from_utf16le` once it's stable
                let utf_16 = data
                    .chunks_exact(2)
                    .map(|chunk| {
                        let arr: [u8; 2] = chunk.try_into().unwrap();
                        u16::from_le_bytes(arr)
                    })
                    .collect::<Vec<_>>();

                String::from_utf16(&utf_16)
                    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
            },
        }
    }
}

/// A wrapper around `WlDataOffer`, implementing `DataTransfer`.
#[derive(Debug, Clone)]
pub struct DataOffer {
    mime_types: Arc<[MimeType]>,
    data: WlDataOffer,
    available_actions: WlDndAction,
    data_device_id: ObjectId,
    serial: u32,
    window_id: WindowId,
}

pub(crate) fn dnd_action_winit_to_wl(winit: DndAction) -> WlDndAction {
    match winit {
        DndAction::Move => WlDndAction::Move,
        DndAction::Copy => WlDndAction::Copy,
        DndAction::Ask => WlDndAction::Ask,
        _ => WlDndAction::empty(),
    }
}

pub(crate) fn dnd_action_wl_to_winit(wl: WlDndAction) -> Option<DndAction> {
    match wl {
        WlDndAction::Move => Some(DndAction::Move),
        WlDndAction::Copy => Some(DndAction::Copy),
        WlDndAction::Ask => Some(DndAction::Ask),
        _ => None,
    }
}

impl DataOffer {
    pub(crate) fn transfer_id(&self) -> DataTransferId {
        make_data_transfer_id(self.data_device_id.clone(), self.serial)
    }

    pub(crate) fn first_mime_type(&self) -> Option<&MimeType> {
        self.mime_types.first()
    }

    pub(crate) fn serial(&self) -> u32 {
        self.serial
    }

    pub(crate) fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub(crate) fn set_actions(&self, action_set: &[DndAction]) -> bool {
        let preferred_action = action_set.iter().find_map(|winit| {
            let wl = dnd_action_winit_to_wl(*winit);
            self.available_actions.intersects(wl).then_some(wl)
        });

        let any = preferred_action.is_some();

        let all_actions = action_set
            .iter()
            .copied()
            .map(dnd_action_winit_to_wl)
            .fold(WlDndAction::empty(), BitOr::bitor);

        self.data.set_actions(all_actions, preferred_action.unwrap_or(WlDndAction::empty()));

        any
    }

    pub(crate) fn find_type_dyn<'a>(&'a self, type_: &'a dyn TransferType) -> Option<&'a MimeType> {
        match type_.cast_ref::<MimeType>() {
            Some(mime_type) => Some(mime_type),
            None => {
                let hint = type_.hint()?;
                self.mime_types.iter().find(|mime_type| {
                    mime_type.hint().is_some_and(|haystack| haystack.matches(&hint))
                })
            },
        }
    }
}

impl Deref for DataOffer {
    type Target = WlDataOffer;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl DataTransfer for DataOffer {
    fn for_each_available_type<'this>(
        &'this self,
        func: &'_ mut dyn FnMut(&'this dyn TransferType) -> std::ops::ControlFlow<()>,
    ) {
        let _ = self.mime_types.iter().map(|mime| mime as &dyn TransferType).try_for_each(func);
    }
}

/// Wrapper for [`WlDataSource`], which exposes the types that are advertised by a data
/// transfer operation, along with the data that the source represents
#[derive(Debug)]
pub struct DragSource {
    pub(crate) data_transfer_id: DataTransferId,
    /// The `WlDataSource` generated from `data`.
    ///
    /// This is stored internally, as if this source is dropped then the
    /// drag operation will be cancelled.
    _data_source: SctkDragSource,
    /// The supplied [`DataTransferSend`].
    pub(crate) data: Box<dyn DataTransferSend>,
    pub(crate) selected_action: WlDndAction,
    pub(crate) window_id: WindowId,
    /// (Optionally) an icon for the drag-and-drop operation.
    _icon: Option<WlSurface>,
}

impl DragSource {
    pub(crate) fn new(
        data_transfer_id: DataTransferId,
        data_source: SctkDragSource,
        data: Box<dyn DataTransferSend>,
        icon: Option<WlSurface>,
        window_id: WindowId,
    ) -> Self {
        Self {
            data_transfer_id,
            _data_source: data_source,
            data,
            selected_action: WlDndAction::None,
            window_id,
            _icon: icon,
        }
    }

    /// Per-type data to be sent. See [`DataTransferSend`].
    pub fn data(&mut self) -> &mut dyn DataTransferSend {
        &mut *self.data
    }
}

/// The current state of an in-progress drag-and-drop operation.
#[derive(Debug, Default)]
pub struct DndState {
    receive_drag: Option<DataOffer>,
    send_drag: Option<DragSource>,
}

impl DndState {
    pub(crate) fn receive_drag(&self) -> Option<&DataOffer> {
        self.receive_drag.as_ref()
    }

    pub(crate) fn set_send_drag(&mut self, source: DragSource) {
        self.send_drag = Some(source);
    }

    pub(crate) fn send_drag(&self) -> Option<&DragSource> {
        self.send_drag.as_ref()
    }

    pub(crate) fn set_target_drag_action(&mut self, action: WlDndAction) {
        if let Some(source) = &mut self.send_drag {
            source.selected_action = action;
        }
    }

    /// Returns `true` if a drag operation was in progress, `false` if no drag operation was in
    /// progress.
    pub(crate) fn clear_send_drag(&mut self) -> bool {
        self.send_drag.take().is_some()
    }

    pub(crate) fn send_drag_data_mut(&mut self) -> Option<&mut dyn DataTransferSend> {
        self.send_drag.as_mut().map(|send| send.data())
    }
}

impl DataOfferHandler for WinitState {
    fn source_actions(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        offer: &mut DragOffer,
        actions: WlDndAction,
    ) {
        let _ = actions;
        let _ = offer;
        let _ = qh;
        let _ = conn;
        // Not implemented, but required for `DataDeviceHandler`.
    }

    fn selected_action(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        offer: &mut DragOffer,
        actions: WlDndAction,
    ) {
        let _ = actions;
        let _ = offer;
        let _ = qh;
        let _ = conn;
    }
}

impl DataDeviceHandler for WinitState {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        data_device: &WlDataDevice,
        x: f64,
        y: f64,
        wl_surface: &WlSurface,
    ) {
        let Some(data) = data_device.data::<DataDeviceData>() else {
            return;
        };

        let Some(drag) = data.drag_offer() else {
            // Selections are not yet implemented
            return;
        };

        let window_id = crate::make_wid(wl_surface);

        let current_drag = drag.with_mime_types(|types| DataOffer {
            mime_types: types
                .iter()
                .map(|str| MimeType::parse(str.clone()))
                .collect::<Vec<_>>()
                .into(),
            available_actions: drag.source_actions,
            serial: drag.serial,
            data_device_id: data_device.id(),
            data: drag.inner().clone(),
            window_id,
        });

        current_drag.set_actions(&[]);

        let id = current_drag.transfer_id();

        self.dnd_state.receive_drag = Some(current_drag);

        let scale_factor = self
            .windows
            .borrow()
            .get(&window_id)
            .map(|window| window.lock().unwrap().scale_factor())
            .unwrap_or(1.);
        let position: PhysicalPosition<f64> = LogicalPosition::new(x, y).to_physical(scale_factor);

        self.events_sink.push_window_event(
            WindowEvent::DragEntered { id, position: Some(position) },
            window_id,
        );
    }

    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, data_device: &WlDataDevice) {
        let Some(data) = data_device.data::<DataDeviceData>() else {
            return;
        };

        if let Some(current_drag) = self.dnd_state.receive_drag() {
            self.events_sink.push_window_event(
                WindowEvent::DragLeft { id: current_drag.transfer_id() },
                current_drag.window_id(),
            );

            self.dnd_state.receive_drag = None;
        }

        if let Some(drag) = data.drag_offer() {
            drag.destroy();
        }
        if let Some(selection) = data.selection_offer() {
            selection.destroy();
        }
    }

    fn motion(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        data_device: &WlDataDevice,
        x: f64,
        y: f64,
    ) {
        let Some(data) = data_device.data::<DataDeviceData>() else {
            return;
        };
        let Some(drag) = data.drag_offer() else {
            // Selections (copy/paste) are not yet implemented
            return;
        };

        // `selected_action` should only contain a single flag, but we check with `contains`
        // just in case we or the compositor misunderstood the spec.
        let proposed_action = if drag.selected_action.contains(WlDndAction::Move) {
            Some(DndAction::Move)
        } else if drag.selected_action.contains(WlDndAction::Copy) {
            Some(DndAction::Copy)
        } else if drag.selected_action.contains(WlDndAction::Ask) {
            Some(DndAction::Ask)
        } else {
            None
        };

        let Some(current_drag) = self.dnd_state.receive_drag() else {
            return;
        };

        let window_id = crate::make_wid(&drag.surface);

        let scale_factor = self
            .windows
            .borrow()
            .get(&window_id)
            .map(|window| window.lock().unwrap().scale_factor())
            .unwrap_or(1.);
        let position: PhysicalPosition<f64> = LogicalPosition::new(x, y).to_physical(scale_factor);

        self.events_sink.push_window_event(
            WindowEvent::DragPosition { id: current_drag.transfer_id(), position, proposed_action },
            window_id,
        );
    }

    fn selection(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {
        // We don't handle selections right now.
    }

    fn drop_performed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        data_device: &WlDataDevice,
    ) {
        let Some(data) = data_device.data::<DataDeviceData>() else {
            return;
        };
        let Some(drag) = data.drag_offer() else {
            // Selections (copy/paste) are not yet implemented
            return;
        };

        let Some(current_drag) = self.dnd_state.receive_drag() else {
            return;
        };

        let window_id = crate::make_wid(&drag.surface);

        // `selected_action` should only contain a single flag, but we check with `contains`
        // just in case we or the compositor misunderstood the spec.
        let proposed_action = if drag.selected_action.contains(WlDndAction::Move) {
            Some(DndAction::Move)
        } else if drag.selected_action.contains(WlDndAction::Copy) {
            Some(DndAction::Copy)
        } else if drag.selected_action.contains(WlDndAction::Ask) {
            Some(DndAction::Ask)
        } else {
            None
        };

        self.events_sink.push_window_event(
            WindowEvent::DragDropped { id: current_drag.transfer_id(), proposed_action },
            window_id,
        );

        self.dnd_state.receive_drag = None;

        if let Some(drag) = data.drag_offer() {
            drag.destroy();
        }
        if let Some(selection) = data.selection_offer() {
            selection.destroy();
        }
    }
}

sctk::delegate_data_device!(WinitState);
