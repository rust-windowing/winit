//! Types related to data transfer (drag-and-drop and clipboard).

use std::collections::hash_map::Entry;
use std::ffi::OsStr;
use std::io::{self, BufRead, Cursor, ErrorKind, Write};
use std::ops::{BitOr, Deref};
use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::{fmt, mem};

use calloop::PostAction;
use dpi::{LogicalPosition, PhysicalPosition};
use foldhash::{HashMap, HashSet};
use rustix::pipe::{self, PipeFlags};
use sctk::compositor::Surface;
use sctk::data_device_manager::data_device::{DataDeviceData, DataDeviceHandler};
use sctk::data_device_manager::data_offer::{
    DataOfferHandler, DragOffer, SelectionOffer, receive_to_fd,
};
use sctk::data_device_manager::data_source::{
    CopyPasteSource, DataSourceHandler, DragSource as SctkDragSource,
};
use sctk::data_device_manager::{ReadPipe, WritePipe};
use sctk::reexports::client::backend::ObjectId;
use sctk::shell::WaylandSurface;
use wayland_client::protocol::wl_data_device::WlDataDevice;
use wayland_client::protocol::wl_data_device_manager::DndAction as WlDndAction;
use wayland_client::protocol::wl_data_offer::WlDataOffer;
use wayland_client::protocol::wl_data_source::WlDataSource;
use wayland_client::protocol::wl_shm::Format;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Proxy, QueueHandle};
use winit_core::data_transfer::{
    DataTransfer, DataTransferId, DataTransferSend, SendData, TransferType, TypeHint, TypedData,
};
use winit_core::error::{NotSupportedError, RequestError};
use winit_core::event::WindowEvent;
use winit_core::event_loop::{AsyncRequestSerial, DndAction, DragIcon};
use winit_core::icon::RgbaIcon;
use winit_core::window::WindowId;

use crate::event_loop::UnknownDataTransfer;
use crate::state::WinitState;
use crate::{image_to_buffer, make_data_transfer_id};

#[derive(Default, Debug, PartialEq, Eq, Clone, Hash)]
enum Charset {
    #[default]
    Utf8,
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

#[derive(Debug)]
struct UnexpectedCharsetError<'a>(&'a str);

impl std::error::Error for UnexpectedCharsetError<'_> {}

impl fmt::Display for UnexpectedCharsetError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unsupported charset: {}", self.0)
    }
}

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
    fn from_dyn(type_: &dyn TransferType) -> impl Iterator<Item = Self> {
        let downcast = type_.cast_ref::<Self>().cloned();
        let downcast_failed = downcast.is_none();
        // This filter is a bit hacky, but it's the only way to ensure that we always
        // return the same type.
        let from_hint = downcast_failed
            .then_some(
                Self::MIME_HINT_MAP
                    .iter()
                    .filter(move |(_, haystack)| TransferType::matches(haystack, type_))
                    .map(move |(mime, _)| Self {
                        mime: mime.to_string().into(),
                        hint: type_.hint(),
                    }),
            )
            .into_iter()
            .flatten();

        downcast.into_iter().chain(from_hint)
    }

    // TODO: We should properly parse MIME types using `mime` or a similar crate.
    fn parse_charset(&self) -> Result<Charset, UnexpectedCharsetError<'_>> {
        let Some((_, charset)) = self
            .mime
            .split_once(';')
            .and_then(|(_essence, options)| options.split_once("charset="))
        else {
            return Ok(Default::default());
        };

        let charset = charset.split_once(',').map(|(first, _)| first).unwrap_or(charset).trim();

        if charset == "utf-8" { Ok(Charset::Utf8) } else { Err(UnexpectedCharsetError(charset)) }
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

#[derive(Debug)]
pub struct MimeData {
    mime_type: MimeType,
    result: BytesResult,
}

impl MimeData {
    fn new(mime_type: MimeType, result: BytesResult) -> Self {
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

    fn try_as_uris(&self) -> io::Result<Vec<String>> {
        let data = self.data()?;

        Cursor::new(&data)
            .lines()
            .filter(|result| match result {
                Ok(s) => !s.starts_with('#'),
                // We want to maintain errors, so the final `collect` returns an error too
                Err(_) => true,
            })
            .collect()
    }

    fn try_as_string(&self) -> io::Result<String> {
        let charset = self.mime_type.parse_charset();

        let data = self.data()?;

        match charset {
            Ok(Charset::Utf8) => String::from_utf8(data.to_vec())
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err)),
            Err(e) => Err(io::Error::other(e.to_string())),
        }
    }
}

/// A snapshot of a data transfer offered by the compositor, implementing [`DataTransfer`].
#[derive(Debug, Clone)]
pub struct DataOffer {
    mime_types: Arc<[MimeType]>,
    transfer_id: DataTransferId,
}

impl DataOffer {
    fn new(mime_types: &[String], transfer_id: DataTransferId) -> Self {
        Self {
            mime_types: mime_types.iter().map(|mime| MimeType::parse(mime.clone())).collect(),
            transfer_id,
        }
    }

    fn transfer_id(&self) -> DataTransferId {
        self.transfer_id
    }

    fn first_mime_type(&self) -> Option<&MimeType> {
        self.mime_types.first()
    }

    fn find_type_dyn<'a>(&'a self, type_: &'a dyn TransferType) -> Option<&'a MimeType> {
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

impl DataTransfer for DataOffer {
    fn for_each_available_type<'this>(
        &'this self,
        func: &'_ mut dyn FnMut(&'this dyn TransferType) -> std::ops::ControlFlow<()>,
    ) {
        let _ = self.mime_types.iter().map(|mime| mime as &dyn TransferType).try_for_each(func);
    }
}

#[derive(Debug)]
struct DataFetch {
    id: DataTransferId,
    serial: AsyncRequestSerial,
    window_id: WindowId,
    mime_type: Option<MimeType>,
    buffer: Vec<u8>,
}

impl DataFetch {
    fn new(
        id: DataTransferId,
        serial: AsyncRequestSerial,
        mime_type: MimeType,
        window_id: WindowId,
    ) -> Self {
        Self { id, serial, window_id, mime_type: Some(mime_type), buffer: Vec::new() }
    }

    fn read(&mut self, file: &mut impl io::Read, state: &mut WinitState) -> PostAction {
        let result = match file.read_to_end(&mut self.buffer) {
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                return PostAction::Continue;
            },
            Ok(0) => Ok(mem::take(&mut self.buffer)),
            Ok(_) => {
                return PostAction::Continue;
            },
            Err(e) => Err(Arc::new(e)),
        };

        state.events_sink.push_window_event(
            WindowEvent::DataTransferReceived {
                id: self.id,
                serial: self.serial,
                // `unwrap` is safe here, as completion happens exactly once.
                value: Arc::new(MimeData::new(self.mime_type.take().unwrap(), result)),
            },
            self.window_id,
        );

        state.dispatched_events = true;

        state.data_transfer_state.fetch_completed(self.id, self.serial);

        PostAction::Remove
    }
}

enum Session<'a> {
    Drag(&'a mut DragSession),
    Clipboard(&'a mut ClipboardSession),
}

impl Session<'_> {
    fn data(&self) -> &DataOffer {
        match self {
            Session::Drag(session) => &session.data,
            Session::Clipboard(session) => &session.data,
        }
    }

    fn start_fetch(self, mime_type: String, writefd: OwnedFd, serial: AsyncRequestSerial) {
        match self {
            Session::Drag(session) => session.start_fetch(mime_type, writefd, serial),
            Session::Clipboard(session) => session.start_fetch(mime_type, writefd, serial),
        }
    }

    fn fetch_completed(self, serial: AsyncRequestSerial) {
        match self {
            Session::Drag(session) => session.fetch_completed(serial),
            Session::Clipboard(session) => session.fetch_completed(serial),
        }
    }
}

#[derive(Debug, Default)]
pub struct DataTransferState {
    drags: HashMap<ObjectId, DragSession>,
    clipboards: HashMap<ObjectId, ClipboardSession>,
    send_drag: Option<DragSource>,
    send_clipboard: Option<ClipboardSource>,
    clipboard_serial: u32,
}

impl DataTransferState {
    fn session_mut(&mut self, id: DataTransferId) -> Option<Session<'_>> {
        if let Some(session) = self.drags.values_mut().find(|drag| drag.data.transfer_id() == id) {
            return Some(Session::Drag(session));
        }

        self.clipboards
            .values_mut()
            .find(|clipboard| clipboard.data.transfer_id() == id)
            .map(Session::Clipboard)
    }

    fn fetch_completed(&mut self, id: DataTransferId, serial: AsyncRequestSerial) {
        if let Some(session) = self.session_mut(id) {
            session.fetch_completed(serial);
        }
    }

    fn send_data_mut(&mut self, source: &WlDataSource) -> Option<&mut dyn DataTransferSend> {
        if let Some(drag) =
            self.send_drag.as_mut().filter(|drag| drag.data_source.inner() == source)
        {
            return Some(drag.data.as_mut());
        }

        self.send_clipboard
            .as_mut()
            .filter(|clipboard| clipboard.data_source.inner() == source)
            .map(|clipboard| &mut *clipboard.data)
    }
}

impl WinitState {
    pub(crate) fn fetch_data_transfer(
        &mut self,
        id: DataTransferId,
        type_: &dyn TransferType,
    ) -> Result<AsyncRequestSerial, RequestError> {
        let Some(session) = self.data_transfer_state.session_mut(id) else {
            return Err(RequestError::Ignored);
        };

        let Some(mime_type) = session.data().find_type_dyn(type_) else {
            return Err(RequestError::Ignored);
        };
        let mime_type = mime_type.clone();

        let window_id = match session {
            Session::Drag(session) => session.window_id,
            Session::Clipboard(session) => {
                let seat = session.seat.clone();
                let Some(window_id) = self.clipboard_window(&seat) else {
                    return Err(RequestError::Ignored);
                };

                window_id
            },
        };

        let mime_type_str = mime_type.to_string();

        let (readfd, writefd) =
            pipe::pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).map_err(|e| os_error!(e))?;

        let async_request_serial = AsyncRequestSerial::get();

        let mut fetch = DataFetch::new(id, async_request_serial, mime_type, window_id);

        self.loop_handle
            .insert_source(ReadPipe::from(readfd), move |_, file, state| {
                // SAFETY: We do not overwrite the referent of `file`
                let file = unsafe { file.get_mut() };

                fetch.read(file, state)
            })
            .map_err(|err| os_error!(io::Error::other(err.to_string())))?;

        self.data_transfer_state
            .session_mut(id)
            .expect("the session can't change while the state is borrowed")
            .start_fetch(mime_type_str, writefd, async_request_serial);

        Ok(async_request_serial)
    }

    pub(crate) fn data_transfer(
        &mut self,
        id: DataTransferId,
    ) -> Result<Box<dyn DataTransfer>, RequestError> {
        match self.data_transfer_state.session_mut(id) {
            Some(session) => Ok(Box::new(session.data().clone())),
            None => Err(RequestError::Ignored),
        }
    }

    pub(crate) fn set_valid_dnd_actions(
        &mut self,
        id: DataTransferId,
        actions: &[DndAction],
    ) -> Result<(), RequestError> {
        let Some(Session::Drag(session)) = self.data_transfer_state.session_mut(id) else {
            return Err(os_error!(UnknownDataTransfer(id)).into());
        };

        session.set_actions(actions);

        Ok(())
    }
}

fn encode_uri_list<I>(uri_list: I) -> Vec<u8>
where
    I: IntoIterator,
    I::Item: AsRef<OsStr>,
{
    let mut out = Vec::new();

    for uri in uri_list {
        out.extend_from_slice(OsStr::new(&uri).as_encoded_bytes());
        out.extend_from_slice(b"\r\n");
    }

    out
}

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
        source: &WlDataSource,
        mime: String,
        fd: WritePipe,
    ) {
        let Some(data) = self.data_transfer_state.send_data_mut(source) else {
            // TODO: Is there a way to explicitly express that the data was not sent?
            return;
        };

        let mime = MimeType::parse(mime);

        let Some(send_data) = data.data_for_type(&mime) else {
            return;
        };

        let mut encoder = match send_data {
            SendData::Uris(strings) => Cursor::new(encode_uri_list(strings)),
            SendData::String(str) => match mime.parse_charset() {
                Ok(Charset::Utf8) => Cursor::new(str.into_bytes()),
                Err(e) => {
                    tracing::error!("{e}");
                    return;
                },
            },
            SendData::Bytes(binary) => Cursor::new(binary),
            _ => return,
        };

        let _ = self.loop_handle.insert_source(fd, move |_, file, _| {
            // SAFETY: We only mutate `file` in-place and do not replace and drop it.
            let file = unsafe { file.get_mut() };
            loop {
                let Ok(encoded_bytes) = encoder.fill_buf() else {
                    return PostAction::Remove;
                };

                match file.write(encoded_bytes) {
                    Ok(0) => {
                        break PostAction::Remove;
                    },
                    Ok(consumed) => {
                        encoder.consume(consumed);
                    },
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

    fn cancelled(&mut self, _: &Connection, _: &QueueHandle<Self>, source: &WlDataSource) {
        let send_clipboard = &mut self.data_transfer_state.send_clipboard;
        if send_clipboard.take_if(|clipboard| clipboard.data_source.inner() == source).is_some() {
            return;
        }

        let Some(current_drag) =
            self.data_transfer_state.send_drag.take_if(|drag| drag.data_source.inner() == source)
        else {
            return;
        };

        let window_id = current_drag.window_id;
        let id = current_drag.data_transfer_id;

        self.events_sink.push_window_event(WindowEvent::OutgoingDragCanceled { id }, window_id);
    }

    fn dnd_dropped(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {
        let Some(current_drag) = self.data_transfer_state.send_drag() else {
            return;
        };

        let window_id = current_drag.window_id;
        let id = current_drag.data_transfer_id;
        let selected_action = current_drag.selected_action;

        self.events_sink.push_window_event(
            WindowEvent::OutgoingDragDropped {
                id,
                action: dnd_action_wl_to_winit(selected_action),
            },
            window_id,
        );
    }

    fn dnd_finished(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wayland_client::protocol::wl_data_source::WlDataSource,
    ) {
        self.data_transfer_state.clear_send_drag();
    }

    fn action(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlDataSource,
        action: WlDndAction,
    ) {
        self.data_transfer_state.set_target_drag_action(action);
    }
}

#[derive(Debug)]
struct ClipboardSession {
    offer: SelectionOffer,
    data: DataOffer,
    seat: ObjectId,
    fetches: HashSet<AsyncRequestSerial>,
}

impl ClipboardSession {
    fn start_fetch(&mut self, mime_type: String, writefd: OwnedFd, serial: AsyncRequestSerial) {
        receive_to_fd(self.offer.inner(), mime_type, writefd);
        self.fetches.insert(serial);
    }

    fn fetch_completed(&mut self, serial: AsyncRequestSerial) {
        let removed = self.fetches.remove(&serial);
        debug_assert!(removed, "fetch completed without a matching start_fetch");
    }
}

#[derive(Debug)]
struct ClipboardSource {
    data_source: CopyPasteSource,
    data: Box<dyn DataTransferSend>,
}

impl DataTransferState {
    fn clipboard_id(&self) -> Option<DataTransferId> {
        self.clipboards.values().next().map(|clipboard| clipboard.data.transfer_id())
    }
}

impl WinitState {
    pub(crate) fn clipboard(&self) -> Result<Option<DataTransferId>, RequestError> {
        self.data_device_manager_state.as_ref().ok_or(NotSupportedError::new(
            "Tried to read the clipboard, but data device not enabled",
        ))?;

        Ok(self.data_transfer_state.clipboard_id())
    }

    pub(crate) fn set_clipboard(
        &mut self,
        queue_handle: &QueueHandle<Self>,
        send_data: Box<dyn DataTransferSend>,
    ) -> Result<(), RequestError> {
        let data_device_manager = self.data_device_manager_state.as_ref().ok_or(
            NotSupportedError::new("Tried to set the clipboard, but data device not enabled"),
        )?;

        let mime_types = send_data
            .available_types()
            .into_iter()
            .flat_map(MimeType::from_dyn)
            .collect::<Vec<_>>();

        let data_source = data_device_manager.create_copy_paste_source(queue_handle, mime_types);

        let (data_device, serial) = self
            .seats
            .values()
            .find_map(|seat| Some((seat.data_device()?, seat.latest_serial()?)))
            .ok_or(NotSupportedError::new(
                "Tried to set the clipboard, but no seat has received input",
            ))?;

        data_source.set_selection(data_device, serial);

        self.data_transfer_state.send_clipboard =
            Some(ClipboardSource { data_source, data: send_data });

        Ok(())
    }

    fn clipboard_window(&self, seat: &ObjectId) -> Option<WindowId> {
        let windows = self.windows.borrow();

        windows
            .iter()
            .find_map(|(window_id, window)| {
                window.lock().unwrap().focused_seats().any(|id| id == seat).then_some(*window_id)
            })
            .or_else(|| self.seats.get(seat)?.pointer_data()?.data().focused_window())
            .or_else(|| windows.keys().next().copied())
    }
}

fn dnd_action_winit_to_wl(winit: DndAction) -> WlDndAction {
    match winit {
        DndAction::Move => WlDndAction::Move,
        DndAction::Copy => WlDndAction::Copy,
        DndAction::Ask => WlDndAction::Ask,
        _ => WlDndAction::empty(),
    }
}

fn dnd_action_wl_to_winit(wl: WlDndAction) -> Option<DndAction> {
    match wl {
        WlDndAction::Move => Some(DndAction::Move),
        WlDndAction::Copy => Some(DndAction::Copy),
        WlDndAction::Ask => Some(DndAction::Ask),
        _ => None,
    }
}

#[derive(Debug)]
struct OfferHandle {
    data: WlDataOffer,
    serial: u32,
}

impl Deref for OfferHandle {
    type Target = WlDataOffer;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

#[derive(Debug)]
struct DragSession {
    offer: OfferHandle,
    data: DataOffer,
    window_id: WindowId,
    source_actions: WlDndAction,
    selected_action: WlDndAction,
    accepted: bool,
    fetches: HashSet<AsyncRequestSerial>,
    phase: Phase,
}

#[derive(Debug, PartialEq, Eq)]
enum Phase {
    Hovering,
    Dropped,
    Concluding,
}

impl DragSession {
    fn new(
        offer: OfferHandle,
        data: DataOffer,
        window_id: WindowId,
        source_actions: WlDndAction,
    ) -> Self {
        Self {
            offer,
            data,
            window_id,
            source_actions,
            selected_action: WlDndAction::empty(),
            accepted: false,
            fetches: HashSet::default(),
            phase: Phase::Hovering,
        }
    }

    fn proposed_action(&self) -> Option<DndAction> {
        // `selected_action` should only contain a single flag, but we check with `contains`
        // just in case we or the compositor misunderstood the spec.
        if self.selected_action.contains(WlDndAction::Move) {
            Some(DndAction::Move)
        } else if self.selected_action.contains(WlDndAction::Copy) {
            Some(DndAction::Copy)
        } else if self.selected_action.contains(WlDndAction::Ask) {
            Some(DndAction::Ask)
        } else {
            None
        }
    }

    fn set_actions(&mut self, action_set: &[DndAction]) {
        let preferred_action = action_set.iter().find_map(|winit| {
            let wl = dnd_action_winit_to_wl(*winit);
            self.source_actions.intersects(wl).then_some(wl)
        });

        let all_actions = action_set
            .iter()
            .copied()
            .map(dnd_action_winit_to_wl)
            .fold(WlDndAction::empty(), BitOr::bitor);

        self.offer.set_actions(all_actions, preferred_action.unwrap_or(WlDndAction::empty()));

        // Some compositors won't even send the "dropped" event if no type
        // has been accepted, so we need to accept _something_ here. The
        // application can accept further types by fetching the data, but
        // this will at least mean that waiting until the drop to start
        // fetching data won't prevent the drop from working at all.
        let accepted_type =
            preferred_action.and(self.data.first_mime_type()).map(|mime| mime.to_string());
        self.accepted = accepted_type.is_some();
        self.offer.accept(self.offer.serial, accepted_type);
    }

    fn start_fetch(&mut self, mime_type: String, writefd: OwnedFd, serial: AsyncRequestSerial) {
        self.accepted = true;
        self.offer.accept(self.offer.serial, Some(mime_type.clone()));
        receive_to_fd(&self.offer, mime_type, writefd);
        self.fetches.insert(serial);
    }

    fn fetch_completed(&mut self, serial: AsyncRequestSerial) {
        let removed = self.fetches.remove(&serial);
        debug_assert!(removed, "fetch completed without a matching start_fetch");
    }

    fn ready_to_conclude(&self) -> bool {
        self.phase == Phase::Concluding && self.fetches.is_empty()
    }

    fn conclude(self) {
        if self.accepted && !self.selected_action.is_empty() && self.offer.version() >= 3 {
            self.offer.finish();
        }
        self.offer.destroy();
    }

    fn abort(self) {
        self.offer.destroy();
    }
}

#[derive(Debug)]
pub struct DragSource {
    data_transfer_id: DataTransferId,
    data_source: SctkDragSource,
    data: Box<dyn DataTransferSend>,
    selected_action: WlDndAction,
    window_id: WindowId,
    _icon: Option<Surface>,
}

impl DragSource {
    fn new(
        data_transfer_id: DataTransferId,
        data_source: SctkDragSource,
        data: Box<dyn DataTransferSend>,
        icon: Option<Surface>,
        window_id: WindowId,
    ) -> Self {
        Self {
            data_transfer_id,
            data_source,
            data,
            selected_action: WlDndAction::None,
            window_id,
            _icon: icon,
        }
    }
}

impl WinitState {
    pub(crate) fn start_drag(
        &mut self,
        queue_handle: &QueueHandle<Self>,
        source: WindowId,
        send_data: Box<dyn DataTransferSend>,
        action_mask: &[DndAction],
        icon: Option<DragIcon>,
    ) -> Result<DataTransferId, RequestError> {
        let dnd_actions = action_mask
            .iter()
            .copied()
            .map(dnd_action_winit_to_wl)
            .fold(WlDndAction::empty(), BitOr::bitor);

        let mime_types = send_data
            .available_types()
            .into_iter()
            .flat_map(MimeType::from_dyn)
            .collect::<Vec<_>>();

        let data_device_manager = self
            .data_device_manager_state
            .as_ref()
            .ok_or(NotSupportedError::new("Tried to initiate drag, but data device not enabled"))?;
        let data_source =
            data_device_manager.create_drag_and_drop_source(queue_handle, mime_types, dnd_actions);

        let icon_surface = icon.and_then(|icon| {
            let rgba = icon.icon.cast_ref::<RgbaIcon>()?;

            let width = rgba.width().try_into().ok()?;
            let height = rgba.height().try_into().ok()?;

            let mut pool = self.image_pool.lock().unwrap();
            let buffer =
                image_to_buffer(width, height, rgba.buffer(), Format::Argb8888, &mut pool).ok()?;

            let surface = self.compositor_state.create_surface(queue_handle);
            if surface.version() >= 5 {
                buffer.attach_to(&surface).ok()?;
                surface.offset(icon.offset_x, icon.offset_y);
            } else {
                surface.attach(Some(buffer.wl_buffer()), icon.offset_x, icon.offset_y);
            }

            Some(Surface::from(surface))
        });

        let windows = self.windows.borrow();
        let window = windows
            .get(&source)
            .ok_or(os_error!("Tried to initiate drag, but source window ID was invalid"))?
            .lock()
            .unwrap();

        const NO_POINTER_CAP: &str =
            "Tried to initiate drag, but source window does not have the pointer capability";

        let seat = window
            .focused_seats()
            // HACK: How do we get the correct seat for pointers here?
            .find_map(|seat| self.seats.get(seat).filter(|seat| seat.data_device().is_some()))
            .ok_or(NotSupportedError::new(NO_POINTER_CAP))?;
        let data_device = seat.data_device().ok_or(NotSupportedError::new(NO_POINTER_CAP))?;

        let serial = seat
            .pointer_data()
            .ok_or(NotSupportedError::new(NO_POINTER_CAP))?
            .latest_button_serial()
            .unwrap_or_default();

        data_source.start_drag(
            data_device,
            window.window.wl_surface(),
            icon_surface.as_ref().map(Surface::wl_surface),
            serial,
        );
        let transfer_id = make_data_transfer_id(data_device.inner().id(), serial);

        // For some reason, if we commit before starting the drag then the offset isn't applied.
        // This doesn't seem to be documented anywhere, and it's possible that it's a bug in KDE.
        if let Some(surface) = &icon_surface {
            surface.wl_surface().commit();
        }

        self.data_transfer_state.send_drag =
            Some(DragSource::new(transfer_id, data_source, send_data, icon_surface, source));

        Ok(transfer_id)
    }
}

impl DataTransferState {
    pub(crate) fn settle_drops(&mut self) {
        for session in self.drags.values_mut() {
            if session.phase == Phase::Dropped {
                session.phase = Phase::Concluding;
            }
        }

        let concluded: Vec<ObjectId> = self
            .drags
            .iter()
            .filter(|(_, session)| session.ready_to_conclude())
            .map(|(device, _)| device.clone())
            .collect();

        for device in concluded {
            self.drags.remove(&device).unwrap().conclude();
        }
    }

    fn send_drag(&self) -> Option<&DragSource> {
        self.send_drag.as_ref()
    }

    fn set_target_drag_action(&mut self, action: WlDndAction) {
        if let Some(source) = &mut self.send_drag {
            source.selected_action = action;
        }
    }

    /// Returns `true` if a drag operation was in progress, `false` if no drag operation was in
    /// progress.
    fn clear_send_drag(&mut self) -> bool {
        self.send_drag.take().is_some()
    }
}

impl DataOfferHandler for WinitState {
    fn source_actions(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        offer: &mut DragOffer,
        actions: WlDndAction,
    ) {
        if let Some(session) = self
            .data_transfer_state
            .drags
            .values_mut()
            .find(|session| session.offer.data == *offer.inner())
        {
            session.source_actions = actions;
        }
    }

    fn selected_action(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        offer: &mut DragOffer,
        actions: WlDndAction,
    ) {
        if let Some(session) = self
            .data_transfer_state
            .drags
            .values_mut()
            .find(|session| session.offer.data == *offer.inner())
        {
            session.selected_action = actions;
        }
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
            return;
        };

        let device = data_device.id();
        let window_id = crate::make_wid(wl_surface);

        let view = drag.with_mime_types(|types| {
            DataOffer::new(types, make_data_transfer_id(device.clone(), drag.serial))
        });
        let offer = OfferHandle { data: drag.inner().clone(), serial: drag.serial };

        let id = view.transfer_id();

        let mut session = DragSession::new(offer, view, window_id, drag.source_actions);
        session.set_actions(&[]);

        if let Some(old) = self.data_transfer_state.drags.insert(device, session) {
            old.abort();
        }

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
        if let Entry::Occupied(entry) = self.data_transfer_state.drags.entry(data_device.id()) {
            if entry.get().phase == Phase::Hovering {
                let session = entry.remove();

                self.events_sink.push_window_event(
                    WindowEvent::DragLeft { id: session.data.transfer_id() },
                    session.window_id,
                );

                session.abort();
            }
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
        let Some(session) = self.data_transfer_state.drags.get(&data_device.id()) else {
            return;
        };

        let id = session.data.transfer_id();
        let window_id = session.window_id;
        let proposed_action = session.proposed_action();

        let scale_factor = self
            .windows
            .borrow()
            .get(&window_id)
            .map(|window| window.lock().unwrap().scale_factor())
            .unwrap_or(1.);
        let position: PhysicalPosition<f64> = LogicalPosition::new(x, y).to_physical(scale_factor);

        self.events_sink.push_window_event(
            WindowEvent::DragPosition { id, position, proposed_action },
            window_id,
        );
    }

    fn selection(&mut self, _: &Connection, _: &QueueHandle<Self>, data_device: &WlDataDevice) {
        let device = data_device.id();

        let serial = &mut self.data_transfer_state.clipboard_serial;
        *serial = serial.wrapping_add(1);
        let serial = *serial;

        let offer = data_device.data::<DataDeviceData>().and_then(|data| data.selection_offer());
        let Some(offer) = offer else {
            self.data_transfer_state.clipboards.remove(&device);
            return;
        };

        let Some(seat) = self.seats.iter().find_map(|(seat, state)| {
            (state.data_device()?.inner() == data_device).then(|| seat.clone())
        }) else {
            return;
        };

        let session = ClipboardSession {
            data: offer.with_mime_types(|types| {
                DataOffer::new(types, make_data_transfer_id(offer.inner().id(), serial))
            }),
            offer,
            seat,
            fetches: HashSet::default(),
        };

        self.data_transfer_state.clipboards.insert(device, session);
    }

    fn drop_performed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        data_device: &WlDataDevice,
    ) {
        let Some(session) = self.data_transfer_state.drags.get_mut(&data_device.id()) else {
            return;
        };

        session.phase = Phase::Dropped;

        self.events_sink.push_window_event(
            WindowEvent::DragDropped {
                id: session.data.transfer_id(),
                proposed_action: session.proposed_action(),
            },
            session.window_id,
        );
    }
}
