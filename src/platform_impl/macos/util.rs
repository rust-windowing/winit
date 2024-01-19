use icrate::Foundation::{NSNotFound, NSRange, NSUInteger};
use log::trace;

// Replace with `!` once stable
#[derive(Debug)]
pub enum Never {}

pub const EMPTY_RANGE: NSRange = NSRange {
    location: NSNotFound as NSUInteger,
    length: 0,
};

macro_rules! trace_scope {
    ($s:literal) => {
        let _crate = $crate::platform_impl::platform::util::TraceGuard::new(module_path!(), $s);
    };
}

pub(crate) struct TraceGuard {
    module_path: &'static str,
    called_from_fn: &'static str,
}

impl TraceGuard {
    #[inline]
    pub(crate) fn new(module_path: &'static str, called_from_fn: &'static str) -> Self {
        trace!(target: module_path, "Triggered `{}`", called_from_fn);
        Self {
            module_path,
            called_from_fn,
        }
    }
}

impl Drop for TraceGuard {
    #[inline]
    fn drop(&mut self) {
        trace!(target: self.module_path, "Completed `{}`", self.called_from_fn);
    }
}
