//! # iOS / UIKit
//!
//! Winit has [the same iOS version requirements as `rustc`][rustc-ios-version], although it's
//! frequently only tested on newer iOS versions.
//!
//! [rustc-ios-version]: https://doc.rust-lang.org/rustc/platform-support/apple-ios.html#os-version
//!
//! ## Running on Mac Catalyst
//!
//! Mac Catalyst allows running applications using UIKit on macOS, which can be very useful for
//! testing. See [`rustc`'s documentation on Mac Catalyst][rustc-mac-catalyst] for details on how to
//! use these targets. To use these with Winit, you'll need to bundle your application before
//! running it, otherwise UIKit will exit with an error.
//!
//! To run e.g. the `window` example in the Winit repository, you can use [`cargo-bundle`] as
//! follows:
//!
//! ```console
//! $ cargo +nightly bundle --format=ios --target=aarch64-apple-ios-macabi --example=window
//! $ ./target/aarch64-apple-ios-macabi/debug/examples/bundle/ios/winit.app/window
//! ```
//!
//! [rustc-mac-catalyst]: https://doc.rust-lang.org/rustc/platform-support/apple-ios-macabi.html
//! [`cargo-bundle`]: https://github.com/burtonageo/cargo-bundle
//!
//! ## Introduction to building an app
//!
//! Building and running your application in the iOS simulator, or on a real device, is a bit more
//! complicated than Mac Catalyst - fundamentally, you must use Xcode, since the binary needs to be
//! bundled, signed, notarized and uploaded to the device (there is [an open source work-in-progress
//! on re-implementing parts of this][apple-platform-rs], but the user-story around it is not yet
//! clear).
//!
//! This means that you're left with effectively two options: Use a tool that manages the Xcode
//! configuration for you, or use Xcode directly. [`cargo-dinghy`] and [`cargo-mobile2`] are notable
//! projects in the ecosystem that attempt the former, and [`cargo-xcode`] is an excellent project
//! that attempts the latter. We will also attempt to describe here how you would go about using
//! Xcode directly:
//!
//! First off, you'll need the correct Rust targets, see [`rustc`'s documentation on iOS][rustc-ios]
//! for details. Nowadays, the correct targets are usually `aarch64-apple-ios-sim` for the
//! simulator, and `aarch64-apple-ios` for the actual device.
//!
//! Next, create a new Xcode project using the "App" template. The exact configuration does not
//! really matter, as we're going to delete most of it, since it's tailored for Objective-C and/or
//! Swift, and Rust/Winit is neither. Specifically, we need to delete:
//! - Everything relating to storyboards (unless you want to use e.g. a launch screen). This
//!   includes the relevant keys in `Info.plist`.
//! - All the generated C header, Objective-C and/or Swift files.
//!
//! Now that we have a fairly clean slate that we can build upon, you can add a "run script" build
//! phase to your Xcode target, which will get invoked instead of the "compile sources" and "link
//! binary" steps. The basic script should look something like:
//!
//! ```sh
//! # Build desired targets based on `ARCHS` environment variable
//! cargo build --target=aarch64-apple-ios --target=armv7s-apple-ios
//! # Merge these with `lipo`, and place the result in "$TARGET_BUILD_DIR/$EXECUTABLE_PATH", which
//! # is understood by Xcode
//! lipo "$TARGET_BUILD_DIR/$EXECUTABLE_PATH" target/aarch64-apple-ios/debug/my_app target/armv7s-apple-ios/debug/my_app
//! ```
//!
//! Note that this is very much the overall idea; the script needs to be much more involved to
//! properly deal with different target architectures, invoking `lipo` when needed, incremental
//! rebuild change detection, and so on. `cargo-xcode` has a script [here][cargo-xcode-script] that
//! handles most of this complexity, you might be able to build upon that.
//!
//! Apologies that we're not able to provide you with more than this; work is in-progress on
//! improving the situation, but it's slow-going.
//!
//! [apple-platform-rs]: https://github.com/indygreg/apple-platform-rs
//! [`cargo-dinghy`]: https://github.com/sonos/dinghy
//! [`cargo-mobile2`]: https://github.com/tauri-apps/cargo-mobile2
//! [`cargo-xcode`]: https://crates.io/crates/cargo-xcode
//! [rustc-ios]: https://doc.rust-lang.org/rustc/platform-support/apple-ios.html
//! [cargo-xcode-script]: https://gitlab.com/kornelski/cargo-xcode/-/blob/main/src/xcodebuild.sh
//!
//! ## App lifecycle and events
//!
//! iOS environment is very different from other platforms and you must be very
//! careful with it's events. Familiarize yourself with
//! [app lifecycle](https://developer.apple.com/library/ios/documentation/UIKit/Reference/UIApplicationDelegate_Protocol/).
//!
//! This is how those event are represented in winit:
//!
//!  - applicationDidBecomeActive is Resumed
//!  - applicationWillResignActive is Suspended
//!  - applicationWillTerminate is LoopExiting
//!
//! Keep in mind that after LoopExiting event is received every attempt to draw with
//! opengl will result in segfault.
//!
//! Also note that app may not receive the LoopExiting event if suspended; it might be SIGKILL'ed.
//!
//! ## Custom `UIApplicationDelegate`
//!
//! Winit usually handles everything related to the lifecycle events of the application. Sometimes,
//! though, you might want to access some of the more niche stuff that [the application
//! delegate][app-delegate] provides. This functionality is not exposed directly in Winit, since it
//! would increase the API surface by quite a lot. Instead, Winit guarantees that it will not
//! register an application delegate, so you can set up a custom one in a nib file instead.
//!
//! [app-delegate]: https://developer.apple.com/documentation/uikit/uiapplicationdelegate?language=objc

use std::os::raw::c_void;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::monitor::{MonitorHandle, VideoModeHandle};
use crate::window::{Window, WindowAttributes};

/// Additional methods on [`Window`] that are specific to iOS.
pub trait WindowExtIOS {
    /// Sets the [`contentScaleFactor`] of the underlying [`UIWindow`] to `scale_factor`.
    ///
    /// The default value is device dependent, and it's recommended GLES or Metal applications set
    /// this to [`MonitorHandle::scale_factor()`].
    ///
    /// [`UIWindow`]: https://developer.apple.com/documentation/uikit/uiwindow?language=objc
    /// [`contentScaleFactor`]: https://developer.apple.com/documentation/uikit/uiview/1622657-contentscalefactor?language=objc
    fn set_scale_factor(&self, scale_factor: f64);

    /// Sets the valid orientations for the [`Window`].
    ///
    /// The default value is [`ValidOrientations::LandscapeAndPortrait`].
    ///
    /// This changes the value returned by
    /// [`-[UIViewController supportedInterfaceOrientations]`](https://developer.apple.com/documentation/uikit/uiviewcontroller/1621435-supportedinterfaceorientations?language=objc),
    /// and then calls
    /// [`-[UIViewController attemptRotationToDeviceOrientation]`](https://developer.apple.com/documentation/uikit/uiviewcontroller/1621400-attemptrotationtodeviceorientati?language=objc).
    fn set_valid_orientations(&self, valid_orientations: ValidOrientations);

    /// Sets whether the [`Window`] prefers the home indicator hidden.
    ///
    /// The default is to prefer showing the home indicator.
    ///
    /// This changes the value returned by
    /// [`-[UIViewController prefersHomeIndicatorAutoHidden]`](https://developer.apple.com/documentation/uikit/uiviewcontroller/2887510-prefershomeindicatorautohidden?language=objc),
    /// and then calls
    /// [`-[UIViewController setNeedsUpdateOfHomeIndicatorAutoHidden]`](https://developer.apple.com/documentation/uikit/uiviewcontroller/2887509-setneedsupdateofhomeindicatoraut?language=objc).
    ///
    /// This only has an effect on iOS 11.0+.
    fn set_prefers_home_indicator_hidden(&self, hidden: bool);

    /// Sets the screen edges for which the system gestures will take a lower priority than the
    /// application's touch handling.
    ///
    /// This changes the value returned by
    /// [`-[UIViewController preferredScreenEdgesDeferringSystemGestures]`](https://developer.apple.com/documentation/uikit/uiviewcontroller/2887512-preferredscreenedgesdeferringsys?language=objc),
    /// and then calls
    /// [`-[UIViewController setNeedsUpdateOfScreenEdgesDeferringSystemGestures]`](https://developer.apple.com/documentation/uikit/uiviewcontroller/2887507-setneedsupdateofscreenedgesdefer?language=objc).
    ///
    /// This only has an effect on iOS 11.0+.
    fn set_preferred_screen_edges_deferring_system_gestures(&self, edges: ScreenEdge);

    /// Sets whether the [`Window`] prefers the status bar hidden.
    ///
    /// The default is to prefer showing the status bar.
    ///
    /// This sets the value of the
    /// [`prefersStatusBarHidden`](https://developer.apple.com/documentation/uikit/uiviewcontroller/1621440-prefersstatusbarhidden?language=objc)
    /// property.
    ///
    /// [`setNeedsStatusBarAppearanceUpdate()`](https://developer.apple.com/documentation/uikit/uiviewcontroller/1621354-setneedsstatusbarappearanceupdat?language=objc)
    /// is also called for you.
    fn set_prefers_status_bar_hidden(&self, hidden: bool);

    /// Sets the preferred status bar style for the [`Window`].
    ///
    /// The default is system-defined.
    ///
    /// This sets the value of the
    /// [`preferredStatusBarStyle`](https://developer.apple.com/documentation/uikit/uiviewcontroller/1621416-preferredstatusbarstyle?language=objc)
    /// property.
    ///
    /// [`setNeedsStatusBarAppearanceUpdate()`](https://developer.apple.com/documentation/uikit/uiviewcontroller/1621354-setneedsstatusbarappearanceupdat?language=objc)
    /// is also called for you.
    fn set_preferred_status_bar_style(&self, status_bar_style: StatusBarStyle);

    /// Sets whether the [`Window`] should recognize pinch gestures.
    ///
    /// The default is to not recognize gestures.
    fn recognize_pinch_gesture(&self, should_recognize: bool);

    /// Sets whether the [`Window`] should recognize pan gestures.
    ///
    /// The default is to not recognize gestures.
    /// Installs [`UIPanGestureRecognizer`](https://developer.apple.com/documentation/uikit/uipangesturerecognizer) onto view
    ///
    /// Set the minimum number of touches required: [`minimumNumberOfTouches`](https://developer.apple.com/documentation/uikit/uipangesturerecognizer/1621208-minimumnumberoftouches)
    ///
    /// Set the maximum number of touches recognized: [`maximumNumberOfTouches`](https://developer.apple.com/documentation/uikit/uipangesturerecognizer/1621208-maximumnumberoftouches)
    fn recognize_pan_gesture(
        &self,
        should_recognize: bool,
        minimum_number_of_touches: u8,
        maximum_number_of_touches: u8,
    );

    /// Sets whether the [`Window`] should recognize double tap gestures.
    ///
    /// The default is to not recognize gestures.
    fn recognize_doubletap_gesture(&self, should_recognize: bool);

    /// Sets whether the [`Window`] should recognize rotation gestures.
    ///
    /// The default is to not recognize gestures.
    fn recognize_rotation_gesture(&self, should_recognize: bool);
}

impl WindowExtIOS for dyn Window + '_ {
    #[inline]
    fn set_scale_factor(&self, scale_factor: f64) {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(move |w| w.set_scale_factor(scale_factor));
    }

    #[inline]
    fn set_valid_orientations(&self, valid_orientations: ValidOrientations) {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(move |w| w.set_valid_orientations(valid_orientations));
    }

    #[inline]
    fn set_prefers_home_indicator_hidden(&self, hidden: bool) {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(move |w| w.set_prefers_home_indicator_hidden(hidden));
    }

    #[inline]
    fn set_preferred_screen_edges_deferring_system_gestures(&self, edges: ScreenEdge) {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(move |w| {
            w.set_preferred_screen_edges_deferring_system_gestures(edges)
        });
    }

    #[inline]
    fn set_prefers_status_bar_hidden(&self, hidden: bool) {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(move |w| w.set_prefers_status_bar_hidden(hidden));
    }

    #[inline]
    fn set_preferred_status_bar_style(&self, status_bar_style: StatusBarStyle) {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(move |w| w.set_preferred_status_bar_style(status_bar_style))
    }

    #[inline]
    fn recognize_pinch_gesture(&self, should_recognize: bool) {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(move |w| w.recognize_pinch_gesture(should_recognize));
    }

    #[inline]
    fn recognize_pan_gesture(
        &self,
        should_recognize: bool,
        minimum_number_of_touches: u8,
        maximum_number_of_touches: u8,
    ) {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(move |w| {
            w.recognize_pan_gesture(
                should_recognize,
                minimum_number_of_touches,
                maximum_number_of_touches,
            )
        });
    }

    #[inline]
    fn recognize_doubletap_gesture(&self, should_recognize: bool) {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(move |w| w.recognize_doubletap_gesture(should_recognize));
    }

    #[inline]
    fn recognize_rotation_gesture(&self, should_recognize: bool) {
        let window = self.as_any().downcast_ref::<crate::platform_impl::Window>().unwrap();
        window.maybe_wait_on_main(move |w| w.recognize_rotation_gesture(should_recognize));
    }
}

/// Additional methods on [`WindowAttributes`] that are specific to iOS.
pub trait WindowAttributesExtIOS {
    /// Sets the [`contentScaleFactor`] of the underlying [`UIWindow`] to `scale_factor`.
    ///
    /// The default value is device dependent, and it's recommended GLES or Metal applications set
    /// this to [`MonitorHandle::scale_factor()`].
    ///
    /// [`UIWindow`]: https://developer.apple.com/documentation/uikit/uiwindow?language=objc
    /// [`contentScaleFactor`]: https://developer.apple.com/documentation/uikit/uiview/1622657-contentscalefactor?language=objc
    fn with_scale_factor(self, scale_factor: f64) -> Self;

    /// Sets the valid orientations for the [`Window`].
    ///
    /// The default value is [`ValidOrientations::LandscapeAndPortrait`].
    ///
    /// This sets the initial value returned by
    /// [`-[UIViewController supportedInterfaceOrientations]`](https://developer.apple.com/documentation/uikit/uiviewcontroller/1621435-supportedinterfaceorientations?language=objc).
    fn with_valid_orientations(self, valid_orientations: ValidOrientations) -> Self;

    /// Sets whether the [`Window`] prefers the home indicator hidden.
    ///
    /// The default is to prefer showing the home indicator.
    ///
    /// This sets the initial value returned by
    /// [`-[UIViewController prefersHomeIndicatorAutoHidden]`](https://developer.apple.com/documentation/uikit/uiviewcontroller/2887510-prefershomeindicatorautohidden?language=objc).
    ///
    /// This only has an effect on iOS 11.0+.
    fn with_prefers_home_indicator_hidden(self, hidden: bool) -> Self;

    /// Sets the screen edges for which the system gestures will take a lower priority than the
    /// application's touch handling.
    ///
    /// This sets the initial value returned by
    /// [`-[UIViewController preferredScreenEdgesDeferringSystemGestures]`](https://developer.apple.com/documentation/uikit/uiviewcontroller/2887512-preferredscreenedgesdeferringsys?language=objc).
    ///
    /// This only has an effect on iOS 11.0+.
    fn with_preferred_screen_edges_deferring_system_gestures(self, edges: ScreenEdge) -> Self;

    /// Sets whether the [`Window`] prefers the status bar hidden.
    ///
    /// The default is to prefer showing the status bar.
    ///
    /// This sets the initial value returned by
    /// [`-[UIViewController prefersStatusBarHidden]`](https://developer.apple.com/documentation/uikit/uiviewcontroller/1621440-prefersstatusbarhidden?language=objc).
    fn with_prefers_status_bar_hidden(self, hidden: bool) -> Self;

    /// Sets the style of the [`Window`]'s status bar.
    ///
    /// The default is system-defined.
    ///
    /// This sets the initial value returned by
    /// [`-[UIViewController preferredStatusBarStyle]`](https://developer.apple.com/documentation/uikit/uiviewcontroller/1621416-preferredstatusbarstyle?language=objc),
    fn with_preferred_status_bar_style(self, status_bar_style: StatusBarStyle) -> Self;
}

impl WindowAttributesExtIOS for WindowAttributes {
    #[inline]
    fn with_scale_factor(mut self, scale_factor: f64) -> Self {
        self.platform_specific.scale_factor = Some(scale_factor);
        self
    }

    #[inline]
    fn with_valid_orientations(mut self, valid_orientations: ValidOrientations) -> Self {
        self.platform_specific.valid_orientations = valid_orientations;
        self
    }

    #[inline]
    fn with_prefers_home_indicator_hidden(mut self, hidden: bool) -> Self {
        self.platform_specific.prefers_home_indicator_hidden = hidden;
        self
    }

    #[inline]
    fn with_preferred_screen_edges_deferring_system_gestures(mut self, edges: ScreenEdge) -> Self {
        self.platform_specific.preferred_screen_edges_deferring_system_gestures = edges;
        self
    }

    #[inline]
    fn with_prefers_status_bar_hidden(mut self, hidden: bool) -> Self {
        self.platform_specific.prefers_status_bar_hidden = hidden;
        self
    }

    #[inline]
    fn with_preferred_status_bar_style(mut self, status_bar_style: StatusBarStyle) -> Self {
        self.platform_specific.preferred_status_bar_style = status_bar_style;
        self
    }
}

/// Additional methods on [`MonitorHandle`] that are specific to iOS.
pub trait MonitorHandleExtIOS {
    /// Returns a pointer to the [`UIScreen`] that is used by this monitor.
    ///
    /// [`UIScreen`]: https://developer.apple.com/documentation/uikit/uiscreen?language=objc
    fn ui_screen(&self) -> *mut c_void;

    /// Returns the preferred [`VideoModeHandle`] for this monitor.
    ///
    /// This translates to a call to [`-[UIScreen preferredMode]`](https://developer.apple.com/documentation/uikit/uiscreen/1617823-preferredmode?language=objc).
    fn preferred_video_mode(&self) -> VideoModeHandle;
}

impl MonitorHandleExtIOS for MonitorHandle {
    #[inline]
    fn ui_screen(&self) -> *mut c_void {
        // SAFETY: The marker is only used to get the pointer of the screen
        let mtm = unsafe { objc2_foundation::MainThreadMarker::new_unchecked() };
        objc2::rc::Retained::as_ptr(self.inner.ui_screen(mtm)) as *mut c_void
    }

    #[inline]
    fn preferred_video_mode(&self) -> VideoModeHandle {
        VideoModeHandle { video_mode: self.inner.preferred_video_mode() }
    }
}

/// Valid orientations for a particular [`Window`].
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ValidOrientations {
    /// Excludes `PortraitUpsideDown` on iphone
    #[default]
    LandscapeAndPortrait,

    Landscape,

    /// Excludes `PortraitUpsideDown` on iphone
    Portrait,
}

bitflags::bitflags! {
    /// The [edges] of a screen.
    ///
    /// [edges]: https://developer.apple.com/documentation/uikit/uirectedge?language=objc
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct ScreenEdge: u8 {
        const NONE   = 0;
        const TOP    = 1 << 0;
        const LEFT   = 1 << 1;
        const BOTTOM = 1 << 2;
        const RIGHT  = 1 << 3;
        const ALL = ScreenEdge::TOP.bits() | ScreenEdge::LEFT.bits()
            | ScreenEdge::BOTTOM.bits() | ScreenEdge::RIGHT.bits();
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum StatusBarStyle {
    #[default]
    Default,
    LightContent,
    DarkContent,
}
