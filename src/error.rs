use std::panic::Location;
use std::{error, fmt};

use crate::platform_impl;

/// The error type for when the OS cannot perform the requested operation.
#[derive(Debug)]
pub struct OsError {
    location: &'static Location<'static>,
    error: platform_impl::OsError,
}

impl OsError {
    #[allow(dead_code)]
    #[track_caller] // Allows `Location::caller` to work properly
    pub(crate) fn new(error: platform_impl::OsError) -> OsError {
        OsError {
            location: Location::caller(),
            error,
        }
    }
}

impl fmt::Display for OsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "os error at {}:{}: {}",
            self.location.file(),
            self.location.line(),
            self.error
        )
    }
}

impl error::Error for OsError {}
