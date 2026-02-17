use dpi::PhysicalPosition;

use crate::application::ApplicationHandler;
use crate::event_loop::ActiveEventLoop;
use crate::window::WindowId;

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
        window_id: WindowId,
        action: &str,
    ) {
        let _ = event_loop;
        let _ = window_id;
        let _ = action;
    }

    /// Called when the user clicks on an inactive window to determine whether the click should
    /// also be processed as a normal mouse event.
    ///
    /// This corresponds to the [`acceptsFirstMouse:`] method on `NSView`, which receives the
    /// triggering mouse event. Winit extracts the click position from that event and passes it
    /// here so that the application can make per-click decisions, e.g. accept first mouse for
    /// low-risk actions (selection, scrolling) but reject it for buttons or destructive actions.
    ///
    /// The default implementation returns `true`.
    ///
    /// If this method cannot be called synchronously (e.g. the handler is already in use), the
    /// static `accepts_first_mouse` value from
    /// [`WindowAttributes`][crate::window::WindowAttributes] is used as a fallback.
    ///
    /// [`acceptsFirstMouse:`]: https://developer.apple.com/documentation/appkit/nsview/acceptsfirstmouse(_:)
    #[doc(alias = "acceptsFirstMouse:")]
    fn accepts_first_mouse(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        position: PhysicalPosition<f64>,
    ) -> bool {
        let _ = (event_loop, window_id, position);
        true
    }
}
