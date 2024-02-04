//! Common error types.

use std::{error, fmt};

/// The error type for when the requested operation is not supported by the backend.
#[derive(Clone)]
pub struct NotSupportedError {
    _marker: (),
}

impl Default for NotSupportedError {
    fn default() -> Self {
        Self::new()
    }
}

impl NotSupportedError {
    /// Create a new [`NotSupportedError`].
    #[inline]
    pub fn new() -> NotSupportedError {
        NotSupportedError { _marker: () }
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

impl error::Error for NotSupportedError {}

#[cfg(test)]
mod tests {
    #![allow(clippy::redundant_clone)]

    use super::*;

    // Eat attributes for testing
    #[test]
    fn ensure_fmt_does_not_panic() {
        let _ = format!(
            "{:?}, {}",
            NotSupportedError::new(),
            NotSupportedError::new().clone()
        );
    }
}
