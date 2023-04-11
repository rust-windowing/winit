use std::{error, fmt};

use crate::platform_impl;

// TODO: Rename
/// An error that may be generated when requesting Winit state
#[derive(Debug)]
pub enum ExternalError {
    /// The operation is not supported by the backend.
    NotSupported(NotSupportedError),
    /// The OS cannot perform the operation.
    Os(OsError),
}

/// The error type for when the requested operation is not supported by the backend.
#[derive(Clone)]
pub struct NotSupportedError {
    _marker: (),
}

/// The error type for when the OS cannot perform the requested operation.
#[derive(Debug)]
pub struct OsError {
    line: u32,
    file: &'static str,
    error: platform_impl::OsError,
}

/// A general error that may occur while running the Winit event loop
#[derive(Debug)]
pub enum RunLoopError {
    /// The operation is not supported by the backend.
    NotSupported(NotSupportedError),
    /// The OS cannot perform the operation.
    Os(OsError),
    /// The event loop can't be re-run while it's already running
    AlreadyRunning,
    /// Application has exit with an error status.
    ExitFailure(i32),
}

impl NotSupportedError {
    #[inline]
    #[allow(dead_code)]
    pub(crate) fn new() -> NotSupportedError {
        NotSupportedError { _marker: () }
    }
}

impl OsError {
    #[allow(dead_code)]
    pub(crate) fn new(line: u32, file: &'static str, error: platform_impl::OsError) -> OsError {
        OsError { line, file, error }
    }
}

#[allow(unused_macros)]
macro_rules! os_error {
    ($error:expr) => {{
        crate::error::OsError::new(line!(), file!(), $error)
    }};
}

impl fmt::Display for OsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.pad(&format!(
            "os error at {}:{}: {}",
            self.file, self.line, self.error
        ))
    }
}

impl fmt::Display for ExternalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            ExternalError::NotSupported(e) => e.fmt(f),
            ExternalError::Os(e) => e.fmt(f),
        }
    }
}

impl fmt::Debug for NotSupportedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("NotSupportedError").finish()
    }
}

impl fmt::Display for NotSupportedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.pad("the requested operation is not supported by Winit")
    }
}

impl fmt::Display for RunLoopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            RunLoopError::AlreadyRunning => write!(f, "EventLoop is already running"),
            RunLoopError::NotSupported(e) => e.fmt(f),
            RunLoopError::Os(e) => e.fmt(f),
            RunLoopError::ExitFailure(status) => write!(f, "Exit Failure: {status}"),
        }
    }
}

impl error::Error for OsError {}
impl error::Error for ExternalError {}
impl error::Error for NotSupportedError {}
impl error::Error for RunLoopError {}
