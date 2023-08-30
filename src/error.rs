use std::{error, fmt};

use crate::platform_impl;

/// The error type for when the OS cannot perform the requested operation.
#[derive(Debug)]
pub struct OsError {
    line: u32,
    file: &'static str,
    error: platform_impl::OsError,
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

impl error::Error for OsError {}
