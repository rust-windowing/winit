use std::{
    io,
    os::raw::*,
    path::{Path, PathBuf},
    str::Utf8Error,
    sync::Arc,
};

use percent_encoding::percent_decode;

use super::{ffi, XConnection};
use xcb_dl_util::error::XcbError;
use xcb_dl_util::property::XcbGetPropertyError;

#[derive(Debug)]
pub struct DndAtoms {
    pub aware: ffi::xcb_atom_t,
    pub enter: ffi::xcb_atom_t,
    pub leave: ffi::xcb_atom_t,
    pub drop: ffi::xcb_atom_t,
    pub position: ffi::xcb_atom_t,
    pub status: ffi::xcb_atom_t,
    pub action_private: ffi::xcb_atom_t,
    pub selection: ffi::xcb_atom_t,
    pub finished: ffi::xcb_atom_t,
    pub type_list: ffi::xcb_atom_t,
    pub uri_list: ffi::xcb_atom_t,
    pub none: ffi::xcb_atom_t,
}

impl DndAtoms {
    pub fn new(xconn: &Arc<XConnection>) -> Self {
        DndAtoms {
            aware: xconn.get_atom("XdndAware"),
            enter: xconn.get_atom("XdndEnter"),
            leave: xconn.get_atom("XdndLeave"),
            drop: xconn.get_atom("XdndDrop"),
            position: xconn.get_atom("XdndPosition"),
            status: xconn.get_atom("XdndStatus"),
            action_private: xconn.get_atom("XdndActionPrivate"),
            selection: xconn.get_atom("XdndSelection"),
            finished: xconn.get_atom("XdndFinished"),
            type_list: xconn.get_atom("XdndTypeList"),
            uri_list: xconn.get_atom("text/uri-list"),
            none: xconn.get_atom("None"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum DndState {
    Accepted,
    Rejected,
}

#[derive(Debug)]
pub enum DndDataParseError {
    EmptyData,
    InvalidUtf8(Utf8Error),
    HostnameSpecified(String),
    UnexpectedProtocol(String),
    UnresolvablePath(io::Error),
}

impl From<Utf8Error> for DndDataParseError {
    fn from(e: Utf8Error) -> Self {
        DndDataParseError::InvalidUtf8(e)
    }
}

impl From<io::Error> for DndDataParseError {
    fn from(e: io::Error) -> Self {
        DndDataParseError::UnresolvablePath(e)
    }
}

pub struct Dnd {
    xconn: Arc<XConnection>,
    pub atoms: DndAtoms,
    // Populated by XdndEnter event handler
    pub version: Option<u32>,
    pub type_list: Option<Vec<ffi::xcb_atom_t>>,
    // Populated by XdndPosition event handler
    pub source_window: Option<ffi::xcb_window_t>,
    // Populated by SelectionNotify event handler (triggered by XdndPosition event handler)
    pub result: Option<Result<Vec<PathBuf>, DndDataParseError>>,
}

impl Dnd {
    pub fn new(xconn: Arc<XConnection>) -> Self {
        let atoms = DndAtoms::new(&xconn);
        Dnd {
            xconn,
            atoms,
            version: None,
            type_list: None,
            source_window: None,
            result: None,
        }
    }

    pub fn reset(&mut self) {
        self.version = None;
        self.type_list = None;
        self.source_window = None;
        self.result = None;
    }

    pub unsafe fn send_status(
        &self,
        this_window: ffi::xcb_window_t,
        target_window: ffi::xcb_window_t,
        state: DndState,
    ) {
        let (accepted, action) = match state {
            DndState::Accepted => (1, self.atoms.action_private),
            DndState::Rejected => (0, self.atoms.none),
        };
        let pending = self.xconn.send_client_msg(
            target_window,
            target_window,
            self.atoms.status,
            None,
            [this_window, accepted, 0, 0, action],
        );
        self.xconn.discard(pending);
    }

    pub unsafe fn send_finished(
        &self,
        this_window: ffi::xcb_window_t,
        target_window: ffi::xcb_window_t,
        state: DndState,
    ) -> Result<(), XcbError> {
        let (accepted, action) = match state {
            DndState::Accepted => (1, self.atoms.action_private),
            DndState::Rejected => (0, self.atoms.none),
        };
        let pending = self.xconn.send_client_msg(
            target_window,
            target_window,
            self.atoms.finished,
            None,
            [this_window, accepted, action, 0, 0],
        );
        self.xconn.check_pending1(pending)
    }

    pub unsafe fn get_type_list(
        &self,
        source_window: ffi::xcb_window_t,
    ) -> Result<Vec<ffi::xcb_atom_t>, XcbGetPropertyError> {
        self.xconn
            .get_property(source_window, self.atoms.type_list, ffi::XCB_ATOM_ATOM)
    }

    pub unsafe fn convert_selection(&self, window: ffi::xcb_window_t, time: ffi::xcb_time_t) {
        self.xconn.xcb.xcb_convert_selection(
            self.xconn.c,
            window,
            self.atoms.selection,
            self.atoms.uri_list,
            self.atoms.selection,
            time,
        );
    }

    pub unsafe fn read_data(
        &self,
        window: ffi::xcb_window_t,
    ) -> Result<Vec<c_uchar>, XcbGetPropertyError> {
        self.xconn
            .get_property(window, self.atoms.selection, self.atoms.uri_list)
    }

    pub fn parse_data(&self, data: &mut Vec<c_uchar>) -> Result<Vec<PathBuf>, DndDataParseError> {
        if !data.is_empty() {
            let mut path_list = Vec::new();
            let decoded = percent_decode(data).decode_utf8()?.into_owned();
            for uri in decoded.split("\r\n").filter(|u| !u.is_empty()) {
                // The format is specified as protocol://host/path
                // However, it's typically simply protocol:///path
                let path_str = if uri.starts_with("file://") {
                    let path_str = uri.replace("file://", "");
                    if !path_str.starts_with('/') {
                        // A hostname is specified
                        // Supporting this case is beyond the scope of my mental health
                        return Err(DndDataParseError::HostnameSpecified(path_str));
                    }
                    path_str
                } else {
                    // Only the file protocol is supported
                    return Err(DndDataParseError::UnexpectedProtocol(uri.to_owned()));
                };

                let path = Path::new(&path_str).canonicalize()?;
                path_list.push(path);
            }
            Ok(path_list)
        } else {
            Err(DndDataParseError::EmptyData)
        }
    }
}
