use crate::application::ApplicationHandler;

/// Additional methods on `EventLoop` for platforms whose run method never return.
///
/// The platform lists below describe built-in backends. External backends may also provide
/// this capability; query [`super::EventLoopProvider::into_never_return`] before calling it when
/// the backend is selected dynamically.
pub trait EventLoopExtNeverReturn {
    /// Run the event loop and call `process::exit` once finished.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS**: This registers the callbacks with the system and calls `UIApplicationMain`.
    /// - **macOS**: Unimplemented (TODO: Should call `NSApplicationMain`).
    /// - **Android/Orbital/Wayland/Windows/X11**: Unsupported.
    /// - **Web**: Impossible to support properly.
    fn run_app_never_return<A: ApplicationHandler + 'static>(self, app: A) -> !;
}

/// An owned capability for starting an event loop that never returns.
///
/// Obtained through [`super::EventLoopProvider::into_never_return`]. See
/// [`EventLoopExtNeverReturn::run_app_never_return`] for lifecycle guarantees.
pub trait RunAppNeverReturn: std::fmt::Debug {
    fn run_app_never_return(self: Box<Self>, app: Box<dyn ApplicationHandler>) -> !;
}
