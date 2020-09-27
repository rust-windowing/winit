use super::super::ScaleChangeArgs;

pub struct ScaleChangeDetector(());

impl ScaleChangeDetector {
    pub(crate) fn new<F>(_handler: F) -> Self
    where
        F: 'static + FnMut(ScaleChangeArgs),
    {
        // TODO: Stub, unimplemented (see web_sys for reference).
        Self(())
    }
}
