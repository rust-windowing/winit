use platform_impl;

pub enum ExternalError {
    NotSupported(NotSupportedError),
    Os(OsError),
}

pub struct NotSupportedError {
    _marker: (),
}

pub struct OsError {
    line: u32,
    file: &'static str,
    error: platform_impl::OsError,
}

impl NotSupportedError {
    #[inline]
    pub(crate) fn new() -> NotSupportedError {
        NotSupportedError {
            _marker: ()
        }
    }
}

impl OsError {
    pub(crate) fn new(line: u32, file: &'static str, error: platform_impl::OsError) -> OsError {
        OsError {
            line,
            file,
            error,
        }
    }
}

macro_rules! os_error {
    ($error:expr) => {{
        crate::error::OsError::new(line!(), file!(), $error)
    }}
}
