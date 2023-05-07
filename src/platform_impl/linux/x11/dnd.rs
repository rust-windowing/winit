use std::{
    io,
    os::raw::*,
    path::{Path, PathBuf},
    str::Utf8Error,
    sync::Arc,
};

use percent_encoding::percent_decode;

use super::{ffi, util, XConnection, XError};

#[derive(Debug)]
pub(crate) struct DndAtoms {
    pub enter: ffi::Atom,
    pub leave: ffi::Atom,
    pub drop: ffi::Atom,
    pub position: ffi::Atom,
    pub status: ffi::Atom,
    pub action_private: ffi::Atom,
    pub selection: ffi::Atom,
    pub finished: ffi::Atom,
    pub type_list: ffi::Atom,
    pub uri_list: ffi::Atom,
    pub none: ffi::Atom,
}

impl DndAtoms {
    pub fn new(xconn: &Arc<XConnection>) -> Result<Self, XError> {
        let names = [
            b"XdndEnter\0".as_ptr() as *mut c_char,
            b"XdndLeave\0".as_ptr() as *mut c_char,
            b"XdndDrop\0".as_ptr() as *mut c_char,
            b"XdndPosition\0".as_ptr() as *mut c_char,
            b"XdndStatus\0".as_ptr() as *mut c_char,
            b"XdndActionPrivate\0".as_ptr() as *mut c_char,
            b"XdndSelection\0".as_ptr() as *mut c_char,
            b"XdndFinished\0".as_ptr() as *mut c_char,
            b"XdndTypeList\0".as_ptr() as *mut c_char,
            b"text/uri-list\0".as_ptr() as *mut c_char,
            b"None\0".as_ptr() as *mut c_char,
        ];
        let atoms = unsafe { xconn.get_atoms(&names) }?;
        Ok(DndAtoms {
            enter: atoms[0],
            leave: atoms[1],
            drop: atoms[2],
            position: atoms[3],
            status: atoms[4],
            action_private: atoms[5],
            selection: atoms[6],
            finished: atoms[7],
            type_list: atoms[8],
            uri_list: atoms[9],
            none: atoms[10],
        })
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

pub(crate) struct Dnd {
    xconn: Arc<XConnection>,
    pub atoms: DndAtoms,
    // Populated by XdndEnter event handler
    pub version: Option<c_long>,
    pub type_list: Option<Vec<c_ulong>>,
    // Populated by XdndPosition event handler
    pub source_window: Option<c_ulong>,
    // Populated by SelectionNotify event handler (triggered by XdndPosition event handler)
    pub result: Option<Result<Vec<PathBuf>, DndDataParseError>>,
}

impl Dnd {
    pub fn new(xconn: Arc<XConnection>) -> Result<Self, XError> {
        let atoms = DndAtoms::new(&xconn)?;
        Ok(Dnd {
            xconn,
            atoms,
            version: None,
            type_list: None,
            source_window: None,
            result: None,
        })
    }

    pub fn reset(&mut self) {
        self.version = None;
        self.type_list = None;
        self.source_window = None;
        self.result = None;
    }

    pub unsafe fn send_status(
        &self,
        this_window: c_ulong,
        target_window: c_ulong,
        state: DndState,
    ) -> Result<(), XError> {
        let (accepted, action) = match state {
            DndState::Accepted => (1, self.atoms.action_private as c_long),
            DndState::Rejected => (0, self.atoms.none as c_long),
        };
        self.xconn
            .send_client_msg(
                target_window,
                target_window,
                self.atoms.status,
                None,
                [this_window as c_long, accepted, 0, 0, action],
            )
            .flush()
    }

    pub unsafe fn send_finished(
        &self,
        this_window: c_ulong,
        target_window: c_ulong,
        state: DndState,
    ) -> Result<(), XError> {
        let (accepted, action) = match state {
            DndState::Accepted => (1, self.atoms.action_private as c_long),
            DndState::Rejected => (0, self.atoms.none as c_long),
        };
        self.xconn
            .send_client_msg(
                target_window,
                target_window,
                self.atoms.finished,
                None,
                [this_window as c_long, accepted, action, 0, 0],
            )
            .flush()
    }

    pub unsafe fn get_type_list(
        &self,
        source_window: c_ulong,
    ) -> Result<Vec<ffi::Atom>, util::GetPropertyError> {
        self.xconn
            .get_property(source_window, self.atoms.type_list, ffi::XA_ATOM)
    }

    pub unsafe fn convert_selection(&self, window: c_ulong, time: c_ulong) {
        (self.xconn.xlib.XConvertSelection)(
            self.xconn.display,
            self.atoms.selection,
            self.atoms.uri_list,
            self.atoms.selection,
            window,
            time,
        );
    }

    pub unsafe fn read_data(
        &self,
        window: c_ulong,
    ) -> Result<Vec<c_uchar>, util::GetPropertyError> {
        self.xconn
            .get_property(window, self.atoms.selection, self.atoms.uri_list)
    }

    pub fn parse_data(&self, data: &mut [c_uchar]) -> Result<Vec<PathBuf>, DndDataParseError> {
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
