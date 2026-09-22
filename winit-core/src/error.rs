use std::error::Error;
use std::fmt::{self, Display};

/// A general error that may occur while running or creating
/// the event loop.
#[derive(Debug)]
#[non_exhaustive]
pub enum EventLoopError {
    /// The event loop can't be re-created.
    RecreationAttempt,
    /// Application has exit with an error status.
    ExitFailure(i32),
    /// Got unspecified OS-specific error during the request.
    Os(OsError),
    /// Creating the event loop with the requested configuration is not supported.
    NotSupported(NotSupportedError),
}

impl fmt::Display for EventLoopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecreationAttempt => {
                write!(
                    f,
                    "EventLoop can't be recreated, only a single instance of it is supported (for \
                     cross-platform compatibility)"
                )
            },
            Self::Os(err) => err.fmt(f),
            Self::ExitFailure(status) => write!(f, "Exit Failure: {status}"),
            Self::NotSupported(err) => err.fmt(f),
        }
    }
}

impl Error for EventLoopError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        if let Self::Os(err) = self { err.source() } else { None }
    }
}

impl From<OsError> for EventLoopError {
    fn from(value: OsError) -> Self {
        Self::Os(value)
    }
}

impl From<NotSupportedError> for EventLoopError {
    fn from(value: NotSupportedError) -> Self {
        Self::NotSupported(value)
    }
}

/// Window creation failure
#[derive(Debug)]
#[non_exhaustive]
pub enum CreateWindowError {
    /// [`WindowType::Popup`] is not supported.
    PopupNotSupported,
    /// Invalid input attribute
    InvalidAttribute(InvalidInput),
    /// Got unspecified OS specific error during the request.
    Os(OsError),
}

impl Display for CreateWindowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PopupNotSupported => write!(f, "WindowType::Popup is not supported"),
            Self::InvalidAttribute(InvalidInput { reason }) => {
                write!(f, "Invalid WindowAttributes: {reason}")
            },
            Self::Os(err) => err.fmt(f),
        }
    }
}
impl Error for CreateWindowError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        if let Self::Os(err) = self { err.source() } else { None }
    }
}

impl From<InvalidInput> for CreateWindowError {
    fn from(value: InvalidInput) -> Self {
        Self::InvalidAttribute(value)
    }
}

impl From<OsError> for CreateWindowError {
    fn from(value: OsError) -> Self {
        Self::Os(value)
    }
}

/// A general error that may occur during a request to the windowing system.
#[derive(Debug)]
#[non_exhaustive]
pub enum RequestError {
    /// The request is not supported.
    NotSupported(NotSupportedError),
    /// The request was ignored by the operating system.
    Ignored,
    /// Got unspecified OS specific error during the request.
    Os(OsError),
}

impl Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSupported(err) => err.fmt(f),
            Self::Ignored => write!(f, "The request was ignored"),
            Self::Os(err) => err.fmt(f),
        }
    }
}
impl Error for RequestError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        if let Self::Os(err) = self { err.source() } else { None }
    }
}

impl From<NotSupportedError> for RequestError {
    fn from(value: NotSupportedError) -> Self {
        Self::NotSupported(value)
    }
}

impl From<OsError> for RequestError {
    fn from(value: OsError) -> Self {
        Self::Os(value)
    }
}

/// An input attribute or parameter was not valid
#[derive(Debug)]
pub struct InvalidInput {
    /// The reason why a certain operation is not supported.
    reason: &'static str,
}

impl InvalidInput {
    pub const fn new(reason: &'static str) -> Self {
        Self { reason }
    }
}

impl fmt::Display for InvalidInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid input: {}", self.reason)
    }
}
impl Error for InvalidInput {}

/// The requested operation is not supported.
#[derive(Debug)]
pub struct NotSupportedError {
    /// The reason why a certain operation is not supported.
    reason: &'static str,
}

impl NotSupportedError {
    pub fn new(reason: &'static str) -> Self {
        Self { reason }
    }
}

impl fmt::Display for NotSupportedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Operation is not supported: {}", self.reason)
    }
}
impl Error for NotSupportedError {}

/// An internal failure occurred
#[derive(Debug)]
pub struct InternalError {
    source: &'static str,
}

impl InternalError {
    pub const fn new(source: &'static str) -> Self {
        Self { source }
    }
}

impl fmt::Display for InternalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Internal error: {}", self.source)
    }
}
impl Error for InternalError {}

/// Unclassified error from the OS.
#[derive(Debug)]
pub struct OsError {
    line: u32,
    file: &'static str,
    error: Box<dyn Error + Send + Sync + 'static>,
}

impl OsError {
    pub fn new(
        line: u32,
        file: &'static str,
        error: impl Into<Box<dyn Error + Send + Sync + 'static>>,
    ) -> Self {
        Self { line, file, error: error.into() }
    }
}

impl Display for OsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(&format!("os error at {}:{}: {}", self.file, self.line, self.error))
    }
}
impl Error for OsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.error.as_ref())
    }
}

#[allow(unused_macros)]
macro_rules! os_error {
    ($error:expr) => {{ crate::error::OsError::new(line!(), file!(), $error) }};
}
