use objc2_core_graphics::CGError;
use winit_core::error::OsError;

macro_rules! os_error {
    ($error:expr) => {{ winit_core::error::OsError::new(line!(), file!(), $error) }};
}

#[track_caller]
pub(crate) fn cgerr(err: CGError) -> Result<(), OsError> {
    if err == CGError::Success { Ok(()) } else { Err(os_error!(format!("CGError {err:?}"))) }
}
