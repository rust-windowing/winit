use crate::platform_impl;
use std::{error, fmt};

#[doc(inline)]
pub use winit_core::error::NotSupportedError;

// TODO: Rename
/// An error that may be generated when requesting Winit state
#[derive(Debug)]
pub enum ExternalError {
    /// The operation is not supported by the backend.
    NotSupported(NotSupportedError),
    /// The operation was ignored.
    Ignored,
    /// The OS cannot perform the operation.
    Os(OsError),
}

impl From<winit_core::event::InnerSizeIgnored> for ExternalError {
    fn from(_: winit_core::event::InnerSizeIgnored) -> Self {
        Self::Ignored
    }
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
pub enum EventLoopError {
    /// The operation is not supported by the backend.
    NotSupported(NotSupportedError),
    /// The OS cannot perform the operation.
    Os(OsError),
    /// The event loop can't be re-created.
    RecreationAttempt,
    /// Application has exit with an error status.
    ExitFailure(i32),
}

impl From<OsError> for EventLoopError {
    fn from(value: OsError) -> Self {
        Self::Os(value)
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
            ExternalError::Ignored => write!(f, "Operation was ignored"),
            ExternalError::Os(e) => e.fmt(f),
        }
    }
}

impl fmt::Display for EventLoopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            EventLoopError::RecreationAttempt => write!(f, "EventLoop can't be recreated"),
            EventLoopError::NotSupported(e) => e.fmt(f),
            EventLoopError::Os(e) => e.fmt(f),
            EventLoopError::ExitFailure(status) => write!(f, "Exit Failure: {status}"),
        }
    }
}

impl error::Error for OsError {}
impl error::Error for ExternalError {}
impl error::Error for EventLoopError {}

#[cfg(test)]
mod tests {
    #![allow(clippy::redundant_clone)]

    use super::*;

    // Eat attributes for testing
    #[test]
    fn ensure_fmt_does_not_panic() {
        let _ = format!(
            "{:?}, {}",
            ExternalError::NotSupported(NotSupportedError::new()),
            ExternalError::NotSupported(NotSupportedError::new())
        );
    }
}
