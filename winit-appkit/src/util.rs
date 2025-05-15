use objc2_core_graphics::CGError;
use tracing::trace;
use winit_core::error::OsError;

macro_rules! os_error {
    ($error:expr) => {{
        winit_core::error::OsError::new(line!(), file!(), $error)
    }};
}

macro_rules! trace_scope {
    ($s:literal) => {
        let _crate = $crate::util::TraceGuard::new(module_path!(), $s);
    };
}

pub(crate) struct TraceGuard {
    module_path: &'static str,
    called_from_fn: &'static str,
}

impl TraceGuard {
    #[inline]
    pub(crate) fn new(module_path: &'static str, called_from_fn: &'static str) -> Self {
        trace!(target = module_path, "Triggered `{}`", called_from_fn);
        Self { module_path, called_from_fn }
    }
}

impl Drop for TraceGuard {
    #[inline]
    fn drop(&mut self) {
        trace!(target = self.module_path, "Completed `{}`", self.called_from_fn);
    }
}

#[track_caller]
pub(crate) fn cgerr(err: CGError) -> Result<(), OsError> {
    if err == CGError::Success {
        Ok(())
    } else {
        Err(os_error!(format!("CGError {err:?}")))
    }
}
