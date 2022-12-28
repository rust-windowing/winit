use std::{
    io,
    os::raw::*,
    path::{Path, PathBuf},
    str::Utf8Error,
    sync::Arc,
};
use x11rb::{
    connection::Connection,
    protocol::xproto::{self, ConnectionExt as _},
};

use percent_encoding::percent_decode;

use super::{
    atoms::*,
    util::{self, PlErrorExt},
    PlatformError, XConnection,
};

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

    // Populated by XdndEnter event handler
    pub version: Option<u32>,
    pub type_list: Option<Vec<u32>>,
    // Populated by XdndPosition event handler
    pub source_window: Option<xproto::Window>,
    // Populated by SelectionNotify event handler (triggered by XdndPosition event handler)
    pub result: Option<Result<Vec<PathBuf>, DndDataParseError>>,
}

impl Dnd {
    pub fn new(xconn: Arc<XConnection>) -> Self {
        Dnd {
            xconn,
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

    pub fn send_status(
        &self,
        this_window: xproto::Window,
        target_window: xproto::Window,
        state: DndState,
    ) -> Result<(), PlatformError> {
        let (accepted, action) = match state {
            DndState::Accepted => (1, XdndActionPrivate),
            DndState::Rejected => (0, AtomType::None),
        };
        self.xconn
            .send_client_msg(
                target_window,
                target_window,
                self.xconn.atoms[XdndStatus],
                None,
                32,
                [this_window, accepted, 0, 0, self.xconn.atoms[action]],
            )?
            .ignore_error();

        self.xconn.connection.flush().platform()
    }

    pub fn send_finished(
        &self,
        this_window: xproto::Window,
        target_window: xproto::Window,
        state: DndState,
    ) -> Result<(), PlatformError> {
        let (accepted, action) = match state {
            DndState::Accepted => (1, XdndActionPrivate),
            DndState::Rejected => (0, AtomType::None),
        };
        self.xconn
            .send_client_msg(
                target_window,
                target_window,
                self.xconn.atoms[XdndFinished],
                None,
                32,
                [this_window, accepted, self.xconn.atoms[action], 0, 0],
            )?
            .ignore_error();

        self.xconn.connection.flush().platform()
    }

    pub fn get_type_list(
        &self,
        source_window: xproto::Window,
    ) -> Result<Vec<xproto::Atom>, util::GetPropertyError> {
        self.xconn.get_property(
            source_window,
            self.xconn.atoms[XdndTypeList],
            xproto::AtomEnum::ATOM.into(),
        )
    }

    pub fn convert_selection(
        &self,
        window: xproto::Window,
        time: xproto::Timestamp,
    ) -> Result<(), PlatformError> {
        self.xconn
            .connection
            .convert_selection(
                window,
                self.xconn.atoms[XdndSelection],
                self.xconn.atoms[TextUriList],
                self.xconn.atoms[XdndSelection],
                time,
            )?
            .ignore_error();

        Ok(())
    }

    pub fn read_data(
        &self,
        window: xproto::Window,
    ) -> Result<Vec<c_uchar>, util::GetPropertyError> {
        self.xconn.get_property(
            window,
            self.xconn.atoms[XdndSelection],
            self.xconn.atoms[TextUriList],
        )
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
