use crate::application::ApplicationHandler;

/// Additional methods on `EventLoop` that registers it with the system event loop.
///
/// The platform lists below describe built-in backends. External backends may also provide
/// this capability; query [`super::EventLoopProvider::into_register`] before calling it when
/// the backend is selected dynamically.
pub trait EventLoopExtRegister {
    /// Initialize and register the application with the system's event loop, such that the
    /// callbacks will be run later.
    ///
    /// ## Platform-specific
    ///
    /// - **Web**: Once the event loop has been destroyed, it's possible to reinitialize another
    ///   event loop by calling this function again. This can be useful if you want to recreate the
    ///   event loop while the WebAssembly module is still loaded. For example, this can be used to
    ///   recreate the event loop when switching between tabs on a single page application.
    /// - **iOS/macOS**: Unimplemented.
    /// - **Android/Orbital/Wayland/Windows/X11**: Unsupported.
    fn register_app<A: ApplicationHandler + 'static>(self, app: A);
}

/// An owned capability for registering an application with a system event loop.
///
/// Obtained through [`super::EventLoopProvider::into_register`]. Both the provider and
/// handler are owned, so their state may be retained for callbacks after this call returns.
/// See [`EventLoopExtRegister::register_app`] for lifecycle guarantees.
pub trait RegisterApp: std::fmt::Debug {
    fn register_app(self: Box<Self>, app: Box<dyn ApplicationHandler>);
}
