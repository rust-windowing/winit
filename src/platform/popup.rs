//! Extension traits for creating popup windows.

use crate::window::WindowBuilder;
use __private::Sealed;
use raw_window_handle::HasRawWindowHandle;

/// Additional methods on [`WindowBuilder`] to create popup windows.
pub trait WindowBuilderExtPopup: Sealed {
    /// Sets this window to be a popup window for the provided parent window.
    ///
    /// This method is only available on Windows and X11. This has no effect on Wayland.
    fn with_transient_parent(self, parent: impl HasRawWindowHandle) -> WindowBuilder;
}

impl WindowBuilderExtPopup for WindowBuilder {
    fn with_transient_parent(mut self, parent: impl HasRawWindowHandle) -> WindowBuilder {
        let hwnd = parent.raw_window_handle();
        self.platform_specific.owner = Some(hwnd);
        self
    }
}

mod __private {
    use crate::window::WindowBuilder;

    #[doc(hidden)]
    pub trait Sealed {}

    impl Sealed for WindowBuilder {}
}
