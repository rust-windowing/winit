//! Types related to data transfer (drag-and-drop and clipboard) on X11.

use std::collections::VecDeque;
use std::ffi::OsStr;
use std::io;
use std::os::raw::*;
use std::str::Utf8Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use winit_core::data_transfer::{
    DataTransfer, DataTransferId, DataTransferSend, SendData, TransferType, TypeHint, TypedData,
};
use winit_core::error::{NotSupportedError, RequestError};
use winit_core::event::WindowEvent;
use winit_core::event_loop::AsyncRequestSerial;
use winit_core::window::WindowId;
use x11_dl::xlib::{XPropertyEvent, XSelectionEvent, XSelectionRequestEvent};
use x11rb::CURRENT_TIME;
use x11rb::protocol::xproto::{self, ConnectionExt};

use crate::atoms::AtomName::NoneAtom as DndNone;
use crate::atoms::*;
use crate::event_loop::{CookieResultExt, X11Error};
use crate::xdisplay::XConnection;
use crate::{XWindow, util};

/// The maximum number of bytes when we are sending data
const INCR_CHUNK_SIZE_BYTES: usize = 1024;

/// TODO: this is copied from wayland. Is this the right approach for X11 as well?
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

#[derive(Debug, Clone, Copy)]
pub enum DndState {
    Accepted,
    Rejected,
}

#[derive(Debug)]
#[non_exhaustive]
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
            Some(TypeHint::UriList) => String::from_utf8(self.data.clone()).map_err(invalid_data),
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
    pub finished: Option<(XWindow, XWindow)>,
    /// Whether the drag operation is accepted (or `None` if the user never indicated that it's
    /// accepted or rejected)
    // Populated by `Window::accept_drag`/`Window::reject_drag`.
    pub accepted: bool,
}

/// Create a new [`DataTransferId`] from a global counter
fn generate_transfer_id() -> DataTransferId {
    static DATA_TRANSFER_ID: AtomicI64 = AtomicI64::new(0);
    DataTransferId::from_raw(DATA_TRANSFER_ID.fetch_add(1, Ordering::Relaxed))
}

impl Default for DragState {
    fn default() -> Self {
        Self {
            version: Default::default(),
            transfer_id: generate_transfer_id(),
            types: Default::default(),
            source_window: Default::default(),
            target_window: Default::default(),
            pending_fetch_types: Default::default(),
            finished: None,
            accepted: Default::default(),
        }
    }
}

/// There are three standard selections defined in the ICCCM: PRIMARY, SECONDARY, and CLIPBOARD.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum ClipboardSelectionType {
    /// Primary selection, often used for the currently selected text
    Primary,
    /// No one ever does anything interesting with SECONDARY as far as I can tell.
    /// src: https://www.freedesktop.org/wiki/Specifications/ClipboardsWiki/
    Secondary,
    /// Clipboard, used for Ctrl+C Ctrl+V pasting
    Clipboard,
}

impl ClipboardSelectionType {
    pub const COUNT: usize = 3;

    /// An atom representing that selection
    #[must_use]
    pub fn selection_atom(self, atoms: &crate::atoms::Atoms) -> xproto::Atom {
        match self {
            Self::Primary => xproto::AtomEnum::PRIMARY.into(),
            Self::Secondary => xproto::AtomEnum::SECONDARY.into(),
            Self::Clipboard => atoms[CLIPBOARD],
        }
    }

    /// Try to convert an atom representing a selection back into a clipboard type
    #[must_use]
    pub fn from_atom(atoms: &crate::atoms::Atoms, atom: xproto::Atom) -> Option<Self> {
        match atom {
            _ if Self::Primary.selection_atom(atoms) == atom => Some(Self::Primary),
            _ if Self::Secondary.selection_atom(atoms) == atom => Some(Self::Secondary),
            _ if Self::Clipboard.selection_atom(atoms) == atom => Some(Self::Clipboard),
            _ => None,
        }
    }

    /// Iterate through all clipboards
    #[must_use]
    pub fn iter() -> impl Iterator<Item = Self> {
        [Self::Primary, Self::Secondary, Self::Clipboard].into_iter()
    }

    /// An atom representing a property that can be used for reading (without conflicting with other
    /// clipboards that might also be reading their TARGETS)
    #[must_use]
    pub fn property_atom(self, atoms: &crate::atoms::Atoms) -> xproto::Atom {
        match self {
            Self::Primary => atoms[WINIT_TARGETS_PRIMARY],
            Self::Secondary => atoms[WINIT_TARGETS_SECONDARY],
            Self::Clipboard => atoms[WINIT_TARGETS_CLIPBOARD],
        }
    }
}

/// State for the INCR clipboard protocol. The protocol works as follows:
/// - Paster calls convert_selection
/// - Copier receives a SelectionNotify event and returns empty data and a type of INCR
/// - Paster deletes the property where the empty data was sent
/// - Copier listens for the delete property message and then puts the first chunk of actual data
/// - Paster listens for the new property message and grabs the data, then deletes the property
///   again
/// - Copier puts then next chunk of data
/// - etc.
/// - When the copier puts an empty chunk of data, the sequence is over
#[derive(Debug)]
struct IncrState {
    /// The partial bytes of data
    data: Vec<u8>,
    /// The place where the INCR protocol takes place (is modified with each chunk) e.g.
    /// CLIPBOARD or META_SELECTION (not necessarily the same as the selection)
    property: xproto::Atom,
    /// The data type transmitted e.g. UTF8_STRING
    ty: SelectionType,
    clipboard: ClipboardSelectionType,
}

#[derive(Debug)]
pub struct ClipboardState {
    /// Data for each [`ClipboardSelection`] if held by this application (data is copied from the
    /// application and sent to others)
    owned_data: Option<Box<dyn DataTransferSend>>,
    /// ID for the current clipboard (each time it is changed this is updated; multiple pastes do
    /// not change this)
    transfer_serial: DataTransferId,
    /// The clipboard notify event serial, used for deduplicating events
    x11_serial: u64,
    /// Cached types available for pasting
    types: Arc<[SelectionType]>,
    /// A list of the fetch requests with the first one being in progress
    pending_fetch_types: VecDeque<(AsyncRequestSerial, SelectionType)>,
}

impl Default for ClipboardState {
    fn default() -> Self {
        Self {
            owned_data: Default::default(),
            transfer_serial: generate_transfer_id(),
            x11_serial: u64::MAX,
            types: Default::default(),
            pending_fetch_types: Default::default(),
        }
    }
}

impl ClipboardState {
    pub fn get_types(&self, atoms: &Atoms) -> Selection {
        if let Some(owned_data) = &self.owned_data {
            let mut accepted: Vec<SelectionType> = Vec::new();

            owned_data.for_each_available_type(&mut |ty| {
                accepted.extend(SelectionType::from_dyn(atoms, ty));
                core::ops::ControlFlow::Continue(())
            });

            return Selection::new(accepted.into());
        }
        Selection::new(Arc::clone(&self.types))
    }
    /// Adds a request to the fech, returning true if the fetch should start immediately
    pub fn add_to_fetch(&mut self, serial: AsyncRequestSerial, ty: SelectionType) -> bool {
        self.pending_fetch_types.push_back((serial, ty));
        self.pending_fetch_types.len() == 1
    }

    /// From the list of types available on the clipboard, find the first available type that
    /// matches the hint.
    pub fn find_type_by_hint(&self, hint: TypeHint, atoms: &Atoms) -> Option<SelectionType> {
        if let Some(owned_data) = &self.owned_data {
            let mut result = None;
            owned_data.for_each_available_type(&mut |ty| {
                if ty.matches(&hint) {
                    result = SelectionType::from_dyn(atoms, ty).next();
                }
                if result.is_some() {
                    core::ops::ControlFlow::Break(())
                } else {
                    core::ops::ControlFlow::Continue(())
                }
            });
            result
        } else {
            self.types.iter().find(|haystack| haystack.hint() == Some(hint)).cloned()
        }
    }
}

#[derive(Debug)]
pub struct DataTransferState {
    xconn: Arc<XConnection>,
    // If `None`, no drag operation is in progress.
    state: Option<DragState>,

    clipboards: [ClipboardState; ClipboardSelectionType::COUNT],
    /// Contains the remenants of the message that are yet to be sent to the current clipboard
    /// recipiant using the INCR ICCCM protocl
    clip_incr_send: Vec<IncrState>,
    /// Contains the partially received message from the clipboard using the INCR ICCCM protocl
    clip_incr_receive: Vec<IncrState>,
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

pub struct FinishedDataTransfer {
    id: DataTransferId,
    serial: AsyncRequestSerial,
    value: Arc<dyn TypedData>,
    window: WindowId,
}

impl FinishedDataTransfer {
    #[must_use]
    pub fn window(&self) -> WindowId {
        self.window
    }
    #[must_use]
    pub fn to_event(self) -> WindowEvent {
        WindowEvent::DataTransferReceived { id: self.id, serial: self.serial, value: self.value }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SelectionType {
    hint: Option<TypeHint>,
    atom: xproto::Atom,
}

impl SelectionType {
    fn atom_hints(atoms: &Atoms) -> impl Iterator<Item = (xproto::Atom, TypeHint)> {
        [
            // Files
            (atoms[TextUriList], TypeHint::UriList),
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
        ]
        .into_iter()
    }

    pub(crate) fn new(atoms: &Atoms, atom: xproto::Atom) -> Self {
        let hint =
            Self::atom_hints(atoms).find_map(|(haystack, hint)| (haystack == atom).then_some(hint));

        Self { hint, atom }
    }

    fn from_dyn(atoms: &Atoms, type_: &dyn TransferType) -> impl Iterator<Item = Self> {
        let downcast = type_.cast_ref::<Self>().cloned();
        let downcast_failed = downcast.is_none();
        // This filter is a bit hacky, but it's the only way to ensure that we always
        // return the same type.
        let from_hint = downcast_failed
            .then_some(
                Self::atom_hints(atoms)
                    .filter(|(_, haystack)| TransferType::matches(haystack, type_))
                    .map(|(atom, hint)| Self { atom, hint: Some(hint) }),
            )
            .into_iter()
            .flatten();

        downcast.into_iter().chain(from_hint)
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

impl DataTransferState {
    pub fn new(xconn: Arc<XConnection>) -> Self {
        // Listen for all selection owner changes for the clipboard. This is a bit hacky but we must
        // know all of the types when the user wants to paste.
        for item in ClipboardSelectionType::iter() {
            let mask: u32 = x11rb::protocol::xfixes::SelectionEventMask::SET_SELECTION_OWNER.into();
            let selection = item.selection_atom(xconn.atoms());
            let root = xconn.default_root().root;

            // TODO: cannot use xcb for unknown reasons as it claims it is an invalid request (even
            // when this works fine with the same parameters?)
            unsafe {
                (xconn.xfixes.XFixesSelectSelectionInput)(
                    xconn.display,
                    root as _,
                    selection as _,
                    mask as _,
                );
            }
        }

        DataTransferState {
            xconn,
            state: Default::default(),
            clipboards: Default::default(), // std::array::from_fn(|_| Default::default()),
            clip_incr_send: Default::default(),
            clip_incr_receive: Default::default(),
        }
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

    /// Gets the [`ClipboardSelectionType`] associated with the specified [`DataTransferId`].
    pub fn resolve_clipboard_type(
        &self,
        data_transfer: DataTransferId,
    ) -> Option<ClipboardSelectionType> {
        ClipboardSelectionType::iter()
            .find(|&clip| self.clipboards[clip as usize].transfer_serial == data_transfer)
    }

    pub fn get_clipboard(&self, clipboard: ClipboardSelectionType) -> &ClipboardState {
        &self.clipboards[clipboard as usize]
    }

    pub fn get_clipboard_mut(&mut self, clipboard: ClipboardSelectionType) -> &mut ClipboardState {
        &mut self.clipboards[clipboard as usize]
    }

    /// Requests to access the clipboard have many properties. Returned true if successfully
    /// resolved.
    pub fn attach_clipboard_property(&mut self, xev: &XSelectionRequestEvent) -> bool {
        let atoms = self.xconn.atoms();
        // For strange reasons, all the values in the input request are u64 whereas the sending uses
        // a different library with u32s
        let prop = xev.property as xproto::Atom;
        let target = xev.target as xproto::Atom;
        let requestor = xev.requestor as xproto::Window;

        let Some(resolved_clipboard) = ClipboardSelectionType::from_atom(atoms, xev.selection as _)
        else {
            warn!("Received request for unknown target clipboard {}", self.xconn.atom_str(prop));
            return false;
        };
        let Some(owned_data) = &self.get_clipboard(resolved_clipboard).owned_data else {
            warn!("Received request for clipboard with no data {:?}", resolved_clipboard);
            return false;
        };

        let replace = xproto::PropMode::REPLACE;

        // Choose the handler for the type of selection
        if target == atoms[TARGETS] as _ {
            // We must advertise at least TIMESTAMP and TARGETS
            let mut accepted: Vec<xproto::Atom> = vec![atoms[TIMESTAMP] as _, atoms[TARGETS] as _];

            // Add user defined types if possible
            owned_data.for_each_available_type(&mut |ty| {
                accepted.extend(SelectionType::from_dyn(atoms, ty).map(|ty| ty.atom()));
                core::ops::ControlFlow::Continue(())
            });

            self.xconn
                .change_property(requestor, prop, xproto::AtomEnum::ATOM.into(), replace, &accepted)
                .expect_then_ignore_error("unable to change property to set TARGETS");

            // Debugging
            let advertised =
                accepted.iter().map(|&atom| self.xconn.atom_str(atom)).collect::<Vec<_>>();
            trace!("Written TARGETS {advertised:?} into property {} ", self.xconn.atom_str(prop));

            true
        } else if target == atoms[TIMESTAMP] as _ {
            self.xconn
                .change_property(requestor, prop, xproto::AtomEnum::INTEGER.into(), replace, &[
                    CURRENT_TIME,
                ])
                .expect_then_ignore_error("unable to change property to set TIMESTAMP");
            trace!("Written TIMESTAMP into property {} ", self.xconn.atom_str(prop));

            true
        } else {
            let ty = SelectionType::new(atoms, target);
            let Some(send_data) = owned_data.data_for_type(&ty) else {
                warn!(
                    "Selection request for unknown selection target {}",
                    self.xconn.atom_str(target)
                );
                return false;
            };
            let data = match send_data {
                SendData::Uris(strings) => encode_uri_list(strings),
                SendData::String(str) => str.into_bytes(),
                SendData::Bytes(binary) => binary,
                _ => {
                    warn!("Unknown send data {send_data:?}");
                    return false;
                },
            };

            if data.len() < INCR_CHUNK_SIZE_BYTES {
                // We can skip using the INCR for short messages.
                self.xconn
                    .change_property(requestor, prop, ty.atom(), replace, &data)
                    .expect_then_ignore_error("unable to change property to send clipboard data");
                trace!("Directly written data of type {} into {} ", self.xconn.atom_str(target), self.xconn.atom_str(prop));
            } else if let Err(e) = self.start_incr_sender(xev, data, ty) {
                warn!("Unable to start INCR sender {e}");
                return false;
            }
            true
        }
    }

    /// Sets up the sending for the INCR
    fn start_incr_sender(
        &mut self,
        xev: &XSelectionRequestEvent,
        data: Vec<u8>,
        ty: SelectionType,
    ) -> Result<(), crate::event_loop::X11Error> {
        let xconn = &self.xconn;
        let atoms = xconn.atoms();

        let property = xev.property as xproto::Atom;
        let requestor = xev.requestor as xproto::Window;
        let Some(clipboard) = ClipboardSelectionType::from_atom(atoms, xev.selection as _) else {
            return Err(X11Error::UnexpectedNull("getting clipboard selection"));
        };

        let current_attributes =
            xconn.xcb_connection().get_window_attributes(requestor)?.reply()?;
        // Add the property change listner to the existing mask so we do not delete bits
        let event_mask =
            Some(current_attributes.your_event_mask | xproto::EventMask::PROPERTY_CHANGE);

        // For INCR we must know when the recipiant deletes the property to add some more
        // Therefore we listen for property change events on the other window
        xconn.xcb_connection().change_window_attributes(
            requestor,
            &xproto::ChangeWindowAttributesAux { event_mask, ..Default::default() },
        )?;
        self.clip_incr_send.push(IncrState { data, property, ty, clipboard });
        xconn
            .change_property::<u32>(requestor, property, atoms[INCR], xproto::PropMode::REPLACE, &[
            ])
            .expect_then_ignore_error("unable to change property to start INCR sender");
        trace!("Starting INCR sender to {requestor} property {}", xconn.atom_str(property));

        Ok(())
    }

    /// Setup an empty buffer to receive data via the INCR protocol
    pub fn start_incr_receiver(&mut self, xev: &XSelectionEvent) {
        let property = xev.property as _;
        let target = xev.target as _;
        let Some(clipboard) =
            ClipboardSelectionType::from_atom(self.xconn.atoms(), xev.selection as _)
        else {
            warn!("Invalid clipboard for starting INCR receiver");
            return;
        };
        let ty = SelectionType::new(self.xconn.atoms(), target);
        trace!(
            "Starting INCR receiver {ty:?} property {} target {}  requestor {}",
            self.xconn.atom_str(property),
            self.xconn.atom_str(target),
            xev.requestor
        );
        self.clip_incr_receive.push(IncrState { data: Vec::new(), property, ty, clipboard });
    }

    /// Check if we are the ones receiving and there is some new value for the clipboard property
    #[must_use]
    pub fn check_incr_receive_data(
        &mut self,
        xev: &XPropertyEvent,
    ) -> Option<FinishedDataTransfer> {
        let atom = xev.atom as xproto::Atom;
        let xwindow = xev.window as xproto::Window;

        // We only care about new values
        if xev.state as u32 != xproto::Property::NEW_VALUE.into() {
            return None;
        }

        let index = self
            .clip_incr_receive
            .iter()
            .position(|IncrState { property, .. }| *property == atom)?;

        let IncrState { data, .. } = &mut self.clip_incr_receive[index];

        // Append the property to the cache
        let (_, read_bytes) = match self.xconn.get_dynamic_property(atom, xwindow, data) {
            Ok(value) => value,
            Err(e) => {
                error!("Unable to read property to check INCR receive: {e}");
                self.clip_incr_receive.remove(index); // Delete if broken to avoid memory leak
                return None;
            },
        };
        // There is still more to come
        if read_bytes != 0 {
            return None;
        }
        // Remove the actual item
        let IncrState { data, property, ty, clipboard } = self.clip_incr_receive.remove(index);
        trace!(
            "We are done receiving from {} ty {}",
            self.xconn.atom_str(property),
            self.xconn.atom_str(ty.atom())
        );

        self.send_clipboard_data(xwindow, clipboard, data)
    }

    /// Start the next clipboard fetch from the queue.
    pub fn do_next_clipboard_fetch(
        &mut self,
        clipboard: ClipboardSelectionType,
        xwindow: xproto::Window,
    ) {
        let selection = clipboard.selection_atom(self.xconn.atoms());
        while let Some((_, ty)) = self.get_clipboard(clipboard).pending_fetch_types.front() {
            match self.xconn.convert_selection(xwindow, selection, ty.atom(), selection) {
                Ok(()) => {
                    trace!("Converting next selection {}", self.xconn.atom_str(ty.atom()));
                    return;
                },
                Err(e) => {
                    // Remove the broken request from the queue
                    self.get_clipboard_mut(clipboard).pending_fetch_types.pop_front();
                    error!("Could not convert next selection so skipping: {e}");
                    // Try the next request
                    continue;
                },
            }
        }
    }

    /// Using the pending clipboard values, send the data to the app with a
    /// [`WindowEvent::DataTransferReceived`].
    #[must_use]
    pub fn send_clipboard_data(
        &mut self,
        xwindow: xproto::Window,
        clipboard: ClipboardSelectionType,
        data: Vec<u8>,
    ) -> Option<FinishedDataTransfer> {
        let Some((serial, type_)) =
            self.get_clipboard_mut(clipboard).pending_fetch_types.pop_front()
        else {
            warn!("Got finished fetch but no fetches were pending");
            return None;
        };
        let value = Arc::new(SelectionReader { type_, data });
        let id = self.get_clipboard(clipboard).transfer_serial;

        self.do_next_clipboard_fetch(clipboard, xwindow);

        let window = crate::event_loop::mkwid(xwindow);
        Some(FinishedDataTransfer { id, serial, value, window })
    }

    /// Check if we are the ones sending and the reciever deleted their property for the clipboard
    /// then update
    pub fn check_incr_send_data(&mut self, xev: &XPropertyEvent) {
        let atom = xev.atom as xproto::Atom;
        let xwindow = xev.window as xproto::Window;

        // We only care about deleted values
        if xev.state as u32 != xproto::Property::DELETE.into() {
            return;
        }

        self.clip_incr_send.retain_mut(|IncrState { data, property, ty, .. }| {
            if *property != atom {
                return true;
            }

            let bytes_to_take = INCR_CHUNK_SIZE_BYTES.min(data.len());
            let sending = &data[..bytes_to_take];
            if let Err(err) = self.xconn.change_property(
                xwindow,
                atom,
                ty.atom(),
                xproto::PropMode::REPLACE,
                sending,
            ) {
                error!("Unable to change property {} for INCR: {}", self.xconn.atom_str(atom), err);
                return false; // Delete broken properties to avoid memory leak
            }
            trace!("Written partial data {:?} to {} ", sending, self.xconn.atom_str(atom),);
            data.drain(..bytes_to_take);
            if bytes_to_take != 0 {
                return true;
            }
            trace!(
                "We are done sending {} ty {}",
                self.xconn.atom_str(*property),
                self.xconn.atom_str(ty.atom())
            );
            false
        });
    }

    /// From the TARGETS request, populate the relevant clipbard
    pub fn populate_targets(&mut self, selection: xproto::Atom, data: Vec<u8>) {
        let (targets_bytes, remainder) = data.as_chunks();
        if !remainder.is_empty() {
            warn!("Trailing bytes in list of targets from X11");
        }
        let targets: Vec<_> = targets_bytes
            .iter()
            .map(|&bytes| xproto::Atom::from_ne_bytes(bytes))
            .map(|atom| SelectionType::new(self.xconn.atoms(), atom))
            .filter(|ty| ty.hint.is_some())
            .collect();

        let Some(clipboard) = ClipboardSelectionType::from_atom(self.xconn.atoms(), selection)
        else {
            warn!("Got TARGETS for an unknown selection {}", self.xconn.atom_str(selection));
            return;
        };

        // Debug
        let available = targets.iter().map(|t| self.xconn.atom_str(t.atom())).collect::<Vec<_>>();
        trace!("Received available targets {available:?} for clipboard {clipboard:?}");

        // Populate selection
        self.get_clipboard_mut(clipboard).types = targets.into();
    }

    pub fn request_updated_targets(
        &mut self,
        clipboard: ClipboardSelectionType,
        xwindow: xproto::Window,
        serial: u64,
    ) {
        let clipboard_state = self.get_clipboard_mut(clipboard);

        // Sometimes you get duplicate update events. Skip these.
        if serial == clipboard_state.x11_serial {
            return;
        }

        // Update serials
        clipboard_state.transfer_serial = generate_transfer_id();
        clipboard_state.x11_serial = serial;

        // If we own the clipboard, there is nothing to do
        if clipboard_state.owned_data.is_some() {
            return;
        }

        self.clear_clipboard(clipboard);

        let atoms = self.xconn.atoms();
        let selection = clipboard.selection_atom(atoms);
        let property = clipboard.property_atom(atoms);
        // Request a list of targets
        if let Err(e) = self.xconn.convert_selection(xwindow, selection, atoms[TARGETS], property) {
            let selection = self.xconn.atom_str(selection);
            let property = self.xconn.atom_str(property);
            error!("Unable to request TARGETS for {selection} into {property}: {e}")
        }
    }

    pub fn request_clipboard_read(&mut self, clipboard: ClipboardSelectionType) -> DataTransferId {
        let clipboard_state = self.get_clipboard(clipboard);
        if let Some(_data) = &clipboard_state.owned_data {
            trace!("Clipboard is owned by current application. TODO: skip X11 protocol");
        }

        clipboard_state.transfer_serial
    }

    /// Update the internal cache of the clipboard contents
    pub fn set_clipboard(
        &mut self,
        xwindow: xproto::Window,
        send_data: Box<dyn DataTransferSend>,
        clipboard: ClipboardSelectionType,
    ) {
        self.get_clipboard_mut(clipboard).owned_data = Some(send_data);
        self.xconn.set_selection_owner(xwindow, clipboard.selection_atom(self.xconn.atoms()));
    }

    pub(crate) fn fetch_dnd_data_transfer(
        &mut self,
        type_: &dyn TransferType,
    ) -> Result<AsyncRequestSerial, RequestError> {
        let type_ = type_
            .cast_ref::<SelectionType>()
            .or_else(|| self.find_type_by_hint(type_.hint()?))
            .cloned()
            .ok_or(RequestError::NotSupported(NotSupportedError::new("Unknown type hint")))?;

        let Some(state) = self.state.as_mut() else {
            return Err(RequestError::Ignored);
        };

        // If it's non-empty, assume that we're still waiting on some other fetch operation.
        // The `SelectionNotify` handler will send a new `convert_selection` event if any
        // more are on the stack.
        let should_emit_convert_selection = state.pending_fetch_types.is_empty();

        let atom = type_.atom();
        let selection = self.xconn.atoms()[XdndSelection];

        if should_emit_convert_selection {
            // TODO: The result is stored into a property with the same name as the selection.
            // Consider using a different name to avoid conflict.
            if let Err(e) =
                self.xconn.convert_selection(state.target_window, selection, atom, selection)
            {
                return Err(RequestError::Os(os_error!(format!(
                    "could not convert selection: {e}"
                ))));
            }
        }

        let serial = AsyncRequestSerial::get();
        state.pending_fetch_types.push_back((serial, type_));

        Ok(serial)
    }

    pub(crate) fn fetch_clipboard_data_transfer(
        &mut self,
        clipboard: ClipboardSelectionType,
        type_: &dyn TransferType,
        xwindow: xproto::Window,
    ) -> Result<AsyncRequestSerial, RequestError> {
        let ty = type_
            .cast_ref::<SelectionType>()
            .cloned()
            .or_else(|| {
                self.get_clipboard(clipboard).find_type_by_hint(type_.hint()?, self.xconn.atoms())
            })
            .ok_or(RequestError::NotSupported(NotSupportedError::new(
                "Requested type is not one of the current available TARGETS of the clipboard",
            )))?;

        let serial = AsyncRequestSerial::get();
        let should_emit_convert_selection =
            self.get_clipboard_mut(clipboard).add_to_fetch(serial, ty);

        if should_emit_convert_selection {
            self.do_next_clipboard_fetch(clipboard, xwindow);
        }

        Ok(serial)
    }

    /// Remove all pending fetches, INCR, and owned data for a clipboard
    pub fn clear_clipboard(&mut self, clipboard: ClipboardSelectionType) {
        let clipboard_state = self.get_clipboard_mut(clipboard);
        if !clipboard_state.pending_fetch_types.is_empty() {
            trace!("Clearing all pending fetch types for {clipboard:?}");
            clipboard_state.pending_fetch_types.clear();
        }

        clipboard_state.types = Default::default();
        if clipboard_state.owned_data.take().is_some() {
            trace!("Cleared owned data for {clipboard:?}")
        }
        self.clip_incr_receive.retain(|receive| receive.clipboard != clipboard);
        self.clip_incr_send.retain(|receive| receive.clipboard != clipboard);
    }
}
