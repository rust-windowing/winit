//! # macOS / AppKit
//!
//! Winit has [the same macOS version requirements as `rustc`][rustc-macos-version], and is tested
//! once in a while on as low as macOS 10.14.
//!
//! [rustc-macos-version]: https://doc.rust-lang.org/rustc/platform-support/apple-darwin.html#os-version
//!
//! ## Custom `NSApplicationDelegate`
//!
//! Winit usually handles everything related to the lifecycle events of the application. Sometimes,
//! though, you might want to do more niche stuff, such as [handle when the user re-activates the
//! application][reopen]. Such functionality is not exposed directly in Winit, since it would
//! increase the API surface by quite a lot.
//!
//! [reopen]: https://developer.apple.com/documentation/appkit/nsapplicationdelegate/1428638-applicationshouldhandlereopen?language=objc
//!
//! Instead, Winit guarantees that it will not register an application delegate, so the solution is
//! to register your own application delegate, as outlined in the following example (see
//! `objc2-app-kit` for more detailed information).
#![cfg_attr(target_os = "macos", doc = "```")]
#![cfg_attr(not(target_os = "macos"), doc = "```ignore")]
//! use objc2::rc::Retained;
//! use objc2::runtime::ProtocolObject;
//! use objc2::{declare_class, msg_send_id, mutability, ClassType, DeclaredClass};
//! use objc2_app_kit::{NSApplication, NSApplicationDelegate};
//! use objc2_foundation::{NSArray, NSURL, MainThreadMarker, NSObject, NSObjectProtocol};
//! use winit::event_loop::EventLoop;
//!
//! declare_class!(
//!     struct AppDelegate;
//!
//!     unsafe impl ClassType for AppDelegate {
//!         type Super = NSObject;
//!         type Mutability = mutability::MainThreadOnly;
//!         const NAME: &'static str = "MyAppDelegate";
//!     }
//!
//!     impl DeclaredClass for AppDelegate {}
//!
//!     unsafe impl NSObjectProtocol for AppDelegate {}
//!
//!     unsafe impl NSApplicationDelegate for AppDelegate {
//!         #[method(application:openURLs:)]
//!         fn application_openURLs(&self, application: &NSApplication, urls: &NSArray<NSURL>) {
//!             // Note: To specifically get `application:openURLs:` to work, you _might_
//!             // have to bundle your application. This is not done in this example.
//!             println!("open urls: {application:?}, {urls:?}");
//!         }
//!     }
//! );
//!
//! impl AppDelegate {
//!     fn new(mtm: MainThreadMarker) -> Retained<Self> {
//!         unsafe { msg_send_id![super(mtm.alloc().set_ivars(())), init] }
//!     }
//! }
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let event_loop = EventLoop::new()?;
//!
//!     let mtm = MainThreadMarker::new().unwrap();
//!     let delegate = AppDelegate::new(mtm);
//!     // Important: Call `sharedApplication` after `EventLoop::new`,
//!     // doing it before is not yet supported.
//!     let app = NSApplication::sharedApplication(mtm);
//!     app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
//!
//!     // event_loop.run_app(&mut my_app);
//!     Ok(())
//! }
//! ```

use std::os::raw::c_void;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::application::ApplicationHandler;
use crate::event_loop::{ActiveEventLoop, EventLoopBuilder};
use crate::monitor::MonitorHandle;
use crate::window::{Window, WindowAttributes, WindowId};

/// Additional methods on [`Window`] that are specific to MacOS.
pub trait WindowExtMacOS {
    /// Returns whether or not the window is in simple fullscreen mode.
    fn simple_fullscreen(&self) -> bool;

    /// Toggles a fullscreen mode that doesn't require a new macOS space.
    /// Returns a boolean indicating whether the transition was successful (this
    /// won't work if the window was already in the native fullscreen).
    ///
    /// This is how fullscreen used to work on macOS in versions before Lion.
    /// And allows the user to have a fullscreen window without using another
    /// space or taking control over the entire monitor.
    fn set_simple_fullscreen(&self, fullscreen: bool) -> bool;

    /// Returns whether or not the window has shadow.
    fn has_shadow(&self) -> bool;

    /// Sets whether or not the window has shadow.
    fn set_has_shadow(&self, has_shadow: bool);

    /// Group windows together by using the same tabbing identifier.
    ///
    /// <https://developer.apple.com/documentation/appkit/nswindow/1644704-tabbingidentifier>
    fn set_tabbing_identifier(&self, identifier: &str);

    /// Returns the window's tabbing identifier.
    fn tabbing_identifier(&self) -> String;

    /// Select next tab.
    fn select_next_tab(&self);

    /// Select previous tab.
    fn select_previous_tab(&self);

    /// Select the tab with the given index.
    ///
    /// Will no-op when the index is out of bounds.
    fn select_tab_at_index(&self, index: usize);

    /// Get the number of tabs in the window tab group.
    fn num_tabs(&self) -> usize;

    /// Get the window's edit state.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// WindowEvent::CloseRequested => {
    ///     if window.is_document_edited() {
    ///         // Show the user a save pop-up or similar
    ///     } else {
    ///         // Close the window
    ///         drop(window);
    ///     }
    /// }
    /// ```
    fn is_document_edited(&self) -> bool;

    /// Put the window in a state which indicates a file save is required.
    fn set_document_edited(&self, edited: bool);

    /// Set option as alt behavior as described in [`OptionAsAlt`].
    ///
    /// This will ignore diacritical marks and accent characters from
    /// being processed as received characters. Instead, the input
    /// device's raw character will be placed in event queues with the
    /// Alt modifier set.
    fn set_option_as_alt(&self, option_as_alt: OptionAsAlt);

    /// Getter for the [`WindowExtMacOS::set_option_as_alt`].
    fn option_as_alt(&self) -> OptionAsAlt;

    /// Disable the Menu Bar and Dock in Borderless Fullscreen mode. Useful for games.
    fn set_borderless_game(&self, borderless_game: bool);

    /// Getter for the [`WindowExtMacOS::set_borderless_game`].
    fn is_borderless_game(&self) -> bool;

    /// Makes the titlebar bigger, effectively adding more space around the
    /// window controls if the titlebar is invisible.
    fn set_unified_titlebar(&self, unified_titlebar: bool);

    /// Getter for the [`WindowExtMacOS::set_unified_titlebar`].
    fn unified_titlebar(&self) -> bool;
}

impl WindowExtMacOS for dyn Window + '_ {
    #[inline]
    fn simple_fullscreen(&self) -> bool {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(|w| w.simple_fullscreen())
    }

    #[inline]
    fn set_simple_fullscreen(&self, fullscreen: bool) -> bool {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(move |w| w.set_simple_fullscreen(fullscreen))
    }

    #[inline]
    fn has_shadow(&self) -> bool {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(|w| w.has_shadow())
    }

    #[inline]
    fn set_has_shadow(&self, has_shadow: bool) {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(move |w| w.set_has_shadow(has_shadow));
    }

    #[inline]
    fn set_tabbing_identifier(&self, identifier: &str) {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(|w| w.set_tabbing_identifier(identifier))
    }

    #[inline]
    fn tabbing_identifier(&self) -> String {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(|w| w.tabbing_identifier())
    }

    #[inline]
    fn select_next_tab(&self) {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(|w| w.select_next_tab());
    }

    #[inline]
    fn select_previous_tab(&self) {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(|w| w.select_previous_tab());
    }

    #[inline]
    fn select_tab_at_index(&self, index: usize) {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(move |w| w.select_tab_at_index(index));
    }

    #[inline]
    fn num_tabs(&self) -> usize {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(|w| w.num_tabs())
    }

    #[inline]
    fn is_document_edited(&self) -> bool {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(|w| w.is_document_edited())
    }

    #[inline]
    fn set_document_edited(&self, edited: bool) {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(move |w| w.set_document_edited(edited));
    }

    #[inline]
    fn set_option_as_alt(&self, option_as_alt: OptionAsAlt) {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(move |w| w.set_option_as_alt(option_as_alt));
    }

    #[inline]
    fn option_as_alt(&self) -> OptionAsAlt {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(|w| w.option_as_alt())
    }

    #[inline]
    fn set_borderless_game(&self, borderless_game: bool) {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(|w| w.set_borderless_game(borderless_game))
    }

    #[inline]
    fn is_borderless_game(&self) -> bool {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(|w| w.is_borderless_game())
    }

    #[inline]
    fn set_unified_titlebar(&self, unified_titlebar: bool) {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(|w| w.set_unified_titlebar(unified_titlebar))
    }

    #[inline]
    fn unified_titlebar(&self) -> bool {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(|w| w.unified_titlebar())
    }
}

/// Corresponds to `NSApplicationActivationPolicy`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ActivationPolicy {
    /// Corresponds to `NSApplicationActivationPolicyRegular`.
    #[default]
    Regular,

    /// Corresponds to `NSApplicationActivationPolicyAccessory`.
    Accessory,

    /// Corresponds to `NSApplicationActivationPolicyProhibited`.
    Prohibited,
}

/// Additional methods on [`WindowAttributes`] that are specific to MacOS.
///
/// **Note:** Properties dealing with the titlebar will be overwritten by the
/// [`WindowAttributes::with_decorations`] method:
/// - `with_titlebar_transparent`
/// - `with_title_hidden`
/// - `with_titlebar_hidden`
/// - `with_titlebar_buttons_hidden`
/// - `with_fullsize_content_view`
pub trait WindowAttributesExtMacOS {
    /// Enables click-and-drag behavior for the entire window, not just the titlebar.
    fn with_movable_by_window_background(self, movable_by_window_background: bool) -> Self;
    /// Makes the titlebar transparent and allows the content to appear behind it.
    fn with_titlebar_transparent(self, titlebar_transparent: bool) -> Self;
    /// Hides the window title.
    fn with_title_hidden(self, title_hidden: bool) -> Self;
    /// Hides the window titlebar.
    fn with_titlebar_hidden(self, titlebar_hidden: bool) -> Self;
    /// Hides the window titlebar buttons.
    fn with_titlebar_buttons_hidden(self, titlebar_buttons_hidden: bool) -> Self;
    /// Makes the window content appear behind the titlebar.
    fn with_fullsize_content_view(self, fullsize_content_view: bool) -> Self;
    fn with_disallow_hidpi(self, disallow_hidpi: bool) -> Self;
    fn with_has_shadow(self, has_shadow: bool) -> Self;
    /// Window accepts click-through mouse events.
    fn with_accepts_first_mouse(self, accepts_first_mouse: bool) -> Self;
    /// Defines the window tabbing identifier.
    ///
    /// <https://developer.apple.com/documentation/appkit/nswindow/1644704-tabbingidentifier>
    fn with_tabbing_identifier(self, identifier: &str) -> Self;
    /// Set how the <kbd>Option</kbd> keys are interpreted.
    ///
    /// See [`WindowExtMacOS::set_option_as_alt`] for details on what this means if set.
    fn with_option_as_alt(self, option_as_alt: OptionAsAlt) -> Self;
    /// See [`WindowExtMacOS::set_borderless_game`] for details on what this means if set.
    fn with_borderless_game(self, borderless_game: bool) -> Self;
    /// See [`WindowExtMacOS::set_unified_titlebar`] for details on what this means if set.
    fn with_unified_titlebar(self, unified_titlebar: bool) -> Self;
}

impl WindowAttributesExtMacOS for WindowAttributes {
    #[inline]
    fn with_movable_by_window_background(mut self, movable_by_window_background: bool) -> Self {
        self.platform_specific.movable_by_window_background = movable_by_window_background;
        self
    }

    #[inline]
    fn with_titlebar_transparent(mut self, titlebar_transparent: bool) -> Self {
        self.platform_specific.titlebar_transparent = titlebar_transparent;
        self
    }

    #[inline]
    fn with_titlebar_hidden(mut self, titlebar_hidden: bool) -> Self {
        self.platform_specific.titlebar_hidden = titlebar_hidden;
        self
    }

    #[inline]
    fn with_titlebar_buttons_hidden(mut self, titlebar_buttons_hidden: bool) -> Self {
        self.platform_specific.titlebar_buttons_hidden = titlebar_buttons_hidden;
        self
    }

    #[inline]
    fn with_title_hidden(mut self, title_hidden: bool) -> Self {
        self.platform_specific.title_hidden = title_hidden;
        self
    }

    #[inline]
    fn with_fullsize_content_view(mut self, fullsize_content_view: bool) -> Self {
        self.platform_specific.fullsize_content_view = fullsize_content_view;
        self
    }

    #[inline]
    fn with_disallow_hidpi(mut self, disallow_hidpi: bool) -> Self {
        self.platform_specific.disallow_hidpi = disallow_hidpi;
        self
    }

    #[inline]
    fn with_has_shadow(mut self, has_shadow: bool) -> Self {
        self.platform_specific.has_shadow = has_shadow;
        self
    }

    #[inline]
    fn with_accepts_first_mouse(mut self, accepts_first_mouse: bool) -> Self {
        self.platform_specific.accepts_first_mouse = accepts_first_mouse;
        self
    }

    #[inline]
    fn with_tabbing_identifier(mut self, tabbing_identifier: &str) -> Self {
        self.platform_specific.tabbing_identifier.replace(tabbing_identifier.to_string());
        self
    }

    #[inline]
    fn with_option_as_alt(mut self, option_as_alt: OptionAsAlt) -> Self {
        self.platform_specific.option_as_alt = option_as_alt;
        self
    }

    #[inline]
    fn with_borderless_game(mut self, borderless_game: bool) -> Self {
        self.platform_specific.borderless_game = borderless_game;
        self
    }

    #[inline]
    fn with_unified_titlebar(mut self, unified_titlebar: bool) -> Self {
        self.platform_specific.unified_titlebar = unified_titlebar;
        self
    }
}

pub trait EventLoopBuilderExtMacOS {
    /// Sets the activation policy for the application. If used, this will override
    /// any relevant settings provided in the package manifest.
    /// For instance, `with_activation_policy(ActivationPolicy::Regular)` will prevent
    /// the application from running as an "agent", even if LSUIElement is set to true.
    ///
    /// If unused, the Winit will honor the package manifest.
    ///
    /// # Example
    ///
    /// Set the activation policy to "accessory".
    ///
    /// ```
    /// use winit::event_loop::EventLoop;
    /// #[cfg(target_os = "macos")]
    /// use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
    ///
    /// let mut builder = EventLoop::builder();
    /// #[cfg(target_os = "macos")]
    /// builder.with_activation_policy(ActivationPolicy::Accessory);
    /// # if false { // We can't test this part
    /// let event_loop = builder.build();
    /// # }
    /// ```
    fn with_activation_policy(&mut self, activation_policy: ActivationPolicy) -> &mut Self;

    /// Used to control whether a default menubar menu is created.
    ///
    /// Menu creation is enabled by default.
    ///
    /// # Example
    ///
    /// Disable creating a default menubar.
    ///
    /// ```
    /// use winit::event_loop::EventLoop;
    /// #[cfg(target_os = "macos")]
    /// use winit::platform::macos::EventLoopBuilderExtMacOS;
    ///
    /// let mut builder = EventLoop::builder();
    /// #[cfg(target_os = "macos")]
    /// builder.with_default_menu(false);
    /// # if false { // We can't test this part
    /// let event_loop = builder.build();
    /// # }
    /// ```
    fn with_default_menu(&mut self, enable: bool) -> &mut Self;

    /// Used to prevent the application from automatically activating when launched if
    /// another application is already active.
    ///
    /// The default behavior is to ignore other applications and activate when launched.
    fn with_activate_ignoring_other_apps(&mut self, ignore: bool) -> &mut Self;
}

impl EventLoopBuilderExtMacOS for EventLoopBuilder {
    #[inline]
    fn with_activation_policy(&mut self, activation_policy: ActivationPolicy) -> &mut Self {
        self.platform_specific.activation_policy = Some(activation_policy);
        self
    }

    #[inline]
    fn with_default_menu(&mut self, enable: bool) -> &mut Self {
        self.platform_specific.default_menu = enable;
        self
    }

    #[inline]
    fn with_activate_ignoring_other_apps(&mut self, ignore: bool) -> &mut Self {
        self.platform_specific.activate_ignoring_other_apps = ignore;
        self
    }
}

/// Additional methods on [`MonitorHandle`] that are specific to MacOS.
pub trait MonitorHandleExtMacOS {
    /// Returns the identifier of the monitor for Cocoa.
    fn native_id(&self) -> u32;
    /// Returns a pointer to the NSScreen representing this monitor.
    fn ns_screen(&self) -> Option<*mut c_void>;
}

impl MonitorHandleExtMacOS for MonitorHandle {
    #[inline]
    fn native_id(&self) -> u32 {
        self.inner.native_identifier()
    }

    fn ns_screen(&self) -> Option<*mut c_void> {
        // SAFETY: We only use the marker to get a pointer
        let mtm = unsafe { objc2_foundation::MainThreadMarker::new_unchecked() };
        self.inner.ns_screen(mtm).map(|s| objc2::rc::Retained::as_ptr(&s) as _)
    }
}

/// Additional methods on [`ActiveEventLoop`] that are specific to macOS.
pub trait ActiveEventLoopExtMacOS {
    /// Hide the entire application. In most applications this is typically triggered with
    /// Command-H.
    fn hide_application(&self);
    /// Hide the other applications. In most applications this is typically triggered with
    /// Command+Option-H.
    fn hide_other_applications(&self);
    /// Set whether the system can automatically organize windows into tabs.
    ///
    /// <https://developer.apple.com/documentation/appkit/nswindow/1646657-allowsautomaticwindowtabbing>
    fn set_allows_automatic_window_tabbing(&self, enabled: bool);
    /// Returns whether the system can automatically organize windows into tabs.
    fn allows_automatic_window_tabbing(&self) -> bool;
}

impl ActiveEventLoopExtMacOS for dyn ActiveEventLoop + '_ {
    fn hide_application(&self) {
        let event_loop = self
            .as_any()
            .downcast_ref::<crate::platform_impl::ActiveEventLoop>()
            .expect("non macOS event loop on macOS");
        event_loop.hide_application()
    }

    fn hide_other_applications(&self) {
        let event_loop = self
            .as_any()
            .downcast_ref::<crate::platform_impl::ActiveEventLoop>()
            .expect("non macOS event loop on macOS");
        event_loop.hide_other_applications()
    }

    fn set_allows_automatic_window_tabbing(&self, enabled: bool) {
        let event_loop = self
            .as_any()
            .downcast_ref::<crate::platform_impl::ActiveEventLoop>()
            .expect("non macOS event loop on macOS");
        event_loop.set_allows_automatic_window_tabbing(enabled);
    }

    fn allows_automatic_window_tabbing(&self) -> bool {
        let event_loop = self
            .as_any()
            .downcast_ref::<crate::platform_impl::ActiveEventLoop>()
            .expect("non macOS event loop on macOS");
        event_loop.allows_automatic_window_tabbing()
    }
}

/// Option as alt behavior.
///
/// The default is `None`.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum OptionAsAlt {
    /// The left `Option` key is treated as `Alt`.
    OnlyLeft,

    /// The right `Option` key is treated as `Alt`.
    OnlyRight,

    /// Both `Option` keys are treated as `Alt`.
    Both,

    /// No special handling is applied for `Option` key.
    #[default]
    None,
}

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
}
