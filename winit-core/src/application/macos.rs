use crate::application::ApplicationHandler;
use crate::event_loop::ActiveEventLoop;
use crate::window::SurfaceId;

/// Additional events on [`ApplicationHandler`] that are specific to macOS.
///
/// This can be registered with [`ApplicationHandler::macos_handler`].
pub trait ApplicationHandlerExtMacOS: ApplicationHandler {
    /// The system interpreted a keypress as a standard key binding command.
    ///
    /// Examples include inserting tabs and newlines, or moving the insertion point, see
    /// [`NSStandardKeyBindingResponding`] for the full list of key bindings. They are often text
    /// editing related.
    ///
    /// This corresponds to the [`doCommandBySelector:`] method on `NSTextInputClient`.
    ///
    /// The `action` parameter contains the string representation of the selector. Examples include
    /// `"insertBacktab:"`, `"indent:"` and `"noop:"`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// impl ApplicationHandlerExtMacOS for App {
    ///     fn standard_key_binding(
    ///         &mut self,
    ///         event_loop: &dyn ActiveEventLoop,
    ///         window_id: WindowId,
    ///         action: &str,
    ///     ) {
    ///         match action {
    ///             "moveBackward:" => self.cursor.position -= 1,
    ///             "moveForward:" => self.cursor.position += 1,
    ///             _ => {} // Ignore other actions
    ///         }
    ///     }
    /// }
    /// ```
    ///
    /// [`NSStandardKeyBindingResponding`]: https://developer.apple.com/documentation/appkit/nsstandardkeybindingresponding?language=objc
    /// [`doCommandBySelector:`]: https://developer.apple.com/documentation/appkit/nstextinputclient/1438256-docommandbyselector?language=objc
    #[doc(alias = "doCommandBySelector:")]
    fn standard_key_binding(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: SurfaceId,
        action: &str,
    ) {
        let _ = event_loop;
        let _ = window_id;
        let _ = action;
    }
}
