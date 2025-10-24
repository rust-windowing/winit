#[allow(unused_macros)]
macro_rules! os_error {
    ($error:expr) => {{ winit_core::error::OsError::new(line!(), file!(), $error) }};
}
