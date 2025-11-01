use crate::application::ApplicationHandler;

/// Additional methods on `EventLoop` that registers it with the system event loop.
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
