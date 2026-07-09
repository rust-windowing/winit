use std::collections::VecDeque;
use std::io;
use std::os::raw::*;
use std::str::Utf8Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use percent_encoding::percent_decode;
use winit_core::data_transfer::{DataTransfer, DataTransferId, TransferType, TypeHint, TypedData};
use winit_core::event_loop::AsyncRequestSerial;
use x11rb::protocol::xproto::{self, ConnectionExt};

use crate::atoms::AtomName::None as DndNone;
use crate::atoms::*;
use crate::event_loop::{CookieResultExt, X11Error};
use crate::util;
use crate::xdisplay::XConnection;

#[derive(Debug, Clone, Copy)]
pub enum DndState {
    Accepted,
    Rejected,
}

#[derive(Debug)]
pub enum UriListParseError {
    EmptyData,
    InvalidUtf8(#[allow(dead_code)] Utf8Error),
    HostnameSpecified(#[allow(dead_code)] String),
    UnexpectedProtocol(#[allow(dead_code)] String),
    UnresolvablePath(#[allow(dead_code)] io::Error),
    Io(#[allow(dead_code)] io::Error),
}

impl From<Utf8Error> for UriListParseError {
    fn from(e: Utf8Error) -> Self {
        UriListParseError::InvalidUtf8(e)
    }
}

impl From<io::Error> for UriListParseError {
    fn from(e: io::Error) -> Self {
        UriListParseError::UnresolvablePath(e)
    }
}

#[derive(Debug)]
pub struct SelectionReader {
    type_: SelectionType,
    data: Vec<u8>,
}

impl TypedData for SelectionReader {
    fn try_read(&self) -> Option<Box<dyn io::BufRead>> {
        Some(Box::new(io::Cursor::new(self.data.clone())))
    }

    fn type_(&self) -> &dyn TransferType {
        &self.type_
    }

    fn try_as_string(&self) -> io::Result<String> {
        fn invalid_data<E>(err: E) -> io::Error
        where
            E: Into<Box<dyn std::error::Error + Send + Sync>>,
        {
            io::Error::new(io::ErrorKind::InvalidData, err)
        }

        fn decode_utf16_bytes(bytes: &[u8]) -> io::Result<String> {
            let utf16 = bytes
                .chunks_exact(2)
                .map(|chunk| {
                    let bytes: &[u8; 2] = chunk.try_into().unwrap();
                    u16::from_ne_bytes(*bytes)
                })
                .collect::<Vec<_>>();
            String::from_utf16(&utf16).map_err(invalid_data)
        }

        match self.type_.hint() {
            Some(TypeHint::Plaintext) | Some(TypeHint::Html) => std::str::from_utf8(&self.data)
                .map(|str| str.to_owned())
                .map_err(invalid_data)
                .or_else(|_| decode_utf16_bytes(&self.data)),
            Some(TypeHint::UriList) => {
                percent_decode(&self.data).decode_utf8().map(Into::into).map_err(invalid_data)
            },
            _ => Err(io::ErrorKind::InvalidData.into()),
        }
    }

    fn try_as_uris(&self) -> io::Result<Vec<String>> {
        if self.type_().hint() != Some(TypeHint::UriList) {
            return Err(io::ErrorKind::InvalidData.into());
        }

        Ok(self
            .try_as_string()?
            .split(['\n', '\r'])
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .collect())
    }
}

#[derive(Debug)]
pub struct DragState {
    // Populated by XdndEnter event handler
    pub version: c_long,
    pub transfer_id: DataTransferId,
    pub types: Arc<[SelectionType]>,
    // Populated by Xdnd* event handlers
    pub source_window: xproto::Window,
    // Populated by Xdnd* event handlers
    pub target_window: xproto::Window,
    // Populated by `fetch_data_transfer`
    pub pending_fetch_types: VecDeque<(AsyncRequestSerial, SelectionType)>,
    /// Whether the drag operation is accepted (or `None` if the user never indicated that it's
    /// accepted or rejected)
    // Populated by `Window::accept_drag`/`Window::reject_drag`.
    pub accepted: bool,
}

impl Default for DragState {
    fn default() -> Self {
        static DATA_TRANSFER_ID: AtomicI64 = AtomicI64::new(0);

        Self {
            version: Default::default(),
            transfer_id: DataTransferId::from_raw(DATA_TRANSFER_ID.fetch_add(1, Ordering::Relaxed)),
            types: Default::default(),
            source_window: Default::default(),
            target_window: Default::default(),
            pending_fetch_types: Default::default(),
            accepted: Default::default(),
        }
    }
}

#[derive(Debug)]
pub struct Dnd {
    xconn: Arc<XConnection>,
    // If `None`, no drag operation is in progress.
    state: Option<DragState>,
}

#[derive(Debug)]
pub struct Selection {
    types: Arc<[SelectionType]>,
}

impl Selection {
    pub(crate) fn new(types: Arc<[SelectionType]>) -> Selection {
        Selection { types }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SelectionType {
    hint: Option<TypeHint>,
    atom: xproto::Atom,
}

impl SelectionType {
    pub(crate) fn new(atoms: &Atoms, atom: xproto::Atom) -> Self {
        let atom_to_hint = [
            // Files
            (atoms[TextUriList], TypeHint::UriList),
            (atoms[TARGETS], TypeHint::UriList),
            (atoms[SAVE_TARGETS], TypeHint::UriList),
            // Plaintext
            (atoms[STRING], TypeHint::Plaintext),
            (atoms[UTF8_STRING], TypeHint::Plaintext),
            (atoms[TextPlain], TypeHint::Plaintext),
            (atoms[TextPlainCharsetUtf8], TypeHint::Plaintext),
            // HTML
            (atoms[TextHtml], TypeHint::Html),
            (atoms[TextHtmlCharsetUtf8], TypeHint::Html),
            // RTF
            (atoms[ApplicationRtf], TypeHint::Rtf),
            // Audio
            (atoms[AudioAac], TypeHint::Audio { extension_hint: Some("aac") }),
            (atoms[AudioAiff], TypeHint::Audio { extension_hint: Some("aif") }),
            (atoms[AudioFlac], TypeHint::Audio { extension_hint: Some("flac") }),
            (atoms[AudioVndWav], TypeHint::Audio { extension_hint: Some("wav") }),
            (atoms[AudioVndWave], TypeHint::Audio { extension_hint: Some("wav") }),
            (atoms[AudioWav], TypeHint::Audio { extension_hint: Some("wav") }),
            (atoms[AudioWave], TypeHint::Audio { extension_hint: Some("wav") }),
            (atoms[AudioXWav], TypeHint::Audio { extension_hint: Some("wav") }),
            (atoms[AudioOgg], TypeHint::Audio { extension_hint: Some("ogg") }),
            (atoms[AudioMpeg], TypeHint::Audio { extension_hint: Some("mp3") }),
            // Image
            (atoms[ImageBmp], TypeHint::Image { extension_hint: Some("bmp") }),
            (atoms[ImageGif], TypeHint::Image { extension_hint: Some("gif") }),
            (atoms[ImageJpeg], TypeHint::Image { extension_hint: Some("jpg") }),
            (atoms[ImagePjpeg], TypeHint::Image { extension_hint: Some("jpg") }),
            (atoms[ImagePng], TypeHint::Image { extension_hint: Some("png") }),
            (atoms[ImageRaw], TypeHint::Image { extension_hint: Some("raw") }),
            (atoms[ImageSvg], TypeHint::Image { extension_hint: Some("svg") }),
            (atoms[ImageTiff], TypeHint::Image { extension_hint: Some("tiff") }),
            (atoms[ImageWebp], TypeHint::Image { extension_hint: Some("webp") }),
            (atoms[ImageXIcon], TypeHint::Image { extension_hint: Some("ico") }),
        ];
        let hint =
            atom_to_hint.iter().find_map(|(haystack, hint)| (*haystack == atom).then_some(*hint));

        Self { hint, atom }
    }

    pub fn atom(&self) -> xproto::Atom {
        self.atom
    }
}

impl TransferType for SelectionType {
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

impl DataTransfer for Selection {
    fn for_each_available_type<'this>(
        &'this self,
        func: &'_ mut dyn FnMut(&'this dyn TransferType) -> std::ops::ControlFlow<()>,
    ) {
        let _ = self.types.iter().map(|mime| mime as &dyn TransferType).try_for_each(func);
    }
}

impl Dnd {
    pub fn new(xconn: Arc<XConnection>) -> Self {
        Dnd { xconn, state: None }
    }

    pub fn state(&self) -> Option<&DragState> {
        self.state.as_ref()
    }

    pub fn state_mut(&mut self) -> Option<&mut DragState> {
        self.state.as_mut()
    }

    pub fn find_type_by_hint(&self, hint: TypeHint) -> Option<&SelectionType> {
        self.state.as_ref()?.types.iter().find(|haystack| haystack.hint() == Some(hint))
    }

    pub fn init_state(
        &mut self,
        version: c_long,
        source_window: xproto::Window,
        target_window: xproto::Window,
        types: Arc<[SelectionType]>,
    ) -> &DragState {
        self.state.get_or_insert(DragState {
            version,
            types,
            source_window,
            target_window,
            ..Default::default()
        })
    }

    pub unsafe fn send_finished(
        &self,
        this_window: xproto::Window,
        target_window: xproto::Window,
    ) -> Result<(), X11Error> {
        let atoms = self.xconn.atoms();
        let Some(state) = &self.state else {
            return Err(X11Error::UnexpectedNull(
                "Drag-and-drop state was not initialized (called `send_finished` before XdndEnter",
            ));
        };
        let (accepted, action) =
            if state.accepted { (1, atoms[XdndActionCopy]) } else { (0, atoms[DndNone]) };
        self.xconn
            .send_client_msg(target_window, target_window, atoms[XdndFinished] as _, None, [
                this_window,
                accepted,
                action as _,
                0,
                0,
            ])?
            .ignore_error();

        Ok(())
    }

    pub unsafe fn get_type_list(
        &self,
        source_window: xproto::Window,
    ) -> Result<Vec<xproto::Atom>, util::GetPropertyError> {
        let atoms = self.xconn.atoms();
        self.xconn.get_property(
            source_window,
            atoms[XdndTypeList],
            xproto::Atom::from(xproto::AtomEnum::ATOM),
        )
    }

    pub fn convert_selection(
        &self,
        window: xproto::Window,
        time: xproto::Timestamp,
        new_type: xproto::Atom,
    ) {
        let atoms = self.xconn.atoms();
        self.xconn
            .xcb_connection()
            // TODO: We store the converted selection back to `XdndSelection`. We should store to
            // some new place so that `XdndSelection` remains untouched.
            .convert_selection(window, atoms[XdndSelection], new_type, atoms[XdndSelection], time)
            .expect_then_ignore_error("Failed to send XdndSelection event")
    }

    pub unsafe fn send_status(
        &self,
        this_window: xproto::Window,
        target_window: xproto::Window,
        status: DndState,
    ) -> Result<(), X11Error> {
        let atoms = self.xconn.atoms();
        let (accepted, action) = match status {
            DndState::Accepted => (1, atoms[XdndActionCopy]),
            DndState::Rejected => (0, atoms[DndNone]),
        };
        self.xconn
            .send_client_msg(target_window, target_window, atoms[XdndStatus] as _, None, [
                this_window,
                accepted,
                0,
                0,
                action as _,
            ])?
            .ignore_error();

        Ok(())
    }

    pub fn read_data(
        &self,
        window: xproto::Window,
        type_: SelectionType,
    ) -> Result<SelectionReader, util::GetPropertyError> {
        let atoms = self.xconn.atoms();
        let type_atom = type_.atom();
        let bytes = self.xconn.get_property(window, atoms[XdndSelection], type_atom)?;

        Ok(SelectionReader { type_, data: bytes })
    }
}
