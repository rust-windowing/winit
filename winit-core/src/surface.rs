//! The [`Window`] trait and associated types.
use std::fmt;

use cursor_icon::CursorIcon;
use dpi::{PhysicalInsets, PhysicalPosition, PhysicalSize, Position, Size};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::as_any::AsAny;
use crate::cursor::Cursor;
use crate::error::RequestError;
use crate::icon::Icon;
use crate::monitor::{Fullscreen, MonitorHandle};

/// Represents a surface that has a window handle.
/// 
/// The surface is closed when dropped.
pub trait Surface: AsAny + Send + Sync + fmt::Debug {
    /// Returns an identifier unique to the window.
    fn id(&self) -> SurfaceId;

    /// Returns the scale factor that can be used to map logical pixels to physical pixels, and
    /// vice versa.
    ///
    /// Note that this value can change depending on user action (for example if the window is
    /// moved to another screen); as such, tracking [`WindowEvent::ScaleFactorChanged`] events is
    /// the most robust way to track the DPI you need to use to draw.
    ///
    /// This value may differ from [`MonitorHandleProvider::scale_factor`].
    ///
    /// See the [`dpi`] crate for more information.
    ///
    /// ## Platform-specific
    ///
    /// The scale factor is calculated differently on different platforms:
    ///
    /// - **Windows:** On Windows 8 and 10, per-monitor scaling is readily configured by users from
    ///   the display settings. While users are free to select any option they want, they're only
    ///   given a selection of "nice" scale factors, i.e. 1.0, 1.25, 1.5... on Windows 7. The scale
    ///   factor is global and changing it requires logging out. See [this article][windows_1] for
    ///   technical details.
    /// - **macOS:** Recent macOS versions allow the user to change the scaling factor for specific
    ///   displays. When available, the user may pick a per-monitor scaling factor from a set of
    ///   pre-defined settings. All "retina displays" have a scaling factor above 1.0 by default,
    ///   but the specific value varies across devices.
    /// - **X11:** Many man-hours have been spent trying to figure out how to handle DPI in X11.
    ///   Winit currently uses a three-pronged approach:
    ///   + Use the value in the `WINIT_X11_SCALE_FACTOR` environment variable if present.
    ///   + If not present, use the value set in `Xft.dpi` in Xresources.
    ///   + Otherwise, calculate the scale factor based on the millimeter monitor dimensions
    ///     provided by XRandR.
    ///
    ///   If `WINIT_X11_SCALE_FACTOR` is set to `randr`, it'll ignore the `Xft.dpi` field and use
    ///   the   XRandR scaling method. Generally speaking, you should try to configure the
    ///   standard system   variables to do what you want before resorting to
    ///   `WINIT_X11_SCALE_FACTOR`.
    /// - **Wayland:** The scale factor is suggested by the compositor for each window individually
    ///   by using the wp-fractional-scale protocol if available. Falls back to integer-scale
    ///   factors otherwise.
    ///
    ///   The monitor scale factor may differ from the window scale factor.
    /// - **iOS:** Scale factors are set by Apple to the value that best suits the device, and range
    ///   from `1.0` to `3.0`. See [this article][apple_1] and [this article][apple_2] for more
    ///   information.
    ///
    ///   This uses the underlying `UIView`'s [`contentScaleFactor`].
    /// - **Android:** Scale factors are set by the manufacturer to the value that best suits the
    ///   device, and range from `1.0` to `4.0`. See [this article][android_1] for more information.
    ///
    ///   This is currently unimplemented, and this function always returns 1.0.
    /// - **Web:** The scale factor is the ratio between CSS pixels and the physical device pixels.
    ///   In other words, it is the value of [`window.devicePixelRatio`][web_1]. It is affected by
    ///   both the screen scaling and the browser zoom level and can go below `1.0`.
    /// - **Orbital:** This is currently unimplemented, and this function always returns 1.0.
    ///
    /// [`WindowEvent::ScaleFactorChanged`]: crate::event::WindowEvent::ScaleFactorChanged
    /// [windows_1]: https://docs.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows
    /// [apple_1]: https://developer.apple.com/library/archive/documentation/DeviceInformation/Reference/iOSDeviceCompatibility/Displays/Displays.html
    /// [apple_2]: https://developer.apple.com/design/human-interface-guidelines/macos/icons-and-images/image-size-and-resolution/
    /// [android_1]: https://developer.android.com/training/multiscreen/screendensities
    /// [web_1]: https://developer.mozilla.org/en-US/docs/Web/API/Window/devicePixelRatio
    /// [`contentScaleFactor`]: https://developer.apple.com/documentation/uikit/uiview/1622657-contentscalefactor?language=objc
    /// [`MonitorHandleProvider::scale_factor`]: crate::monitor::MonitorHandleProvider::scale_factor.
    fn scale_factor(&self) -> f64;

    /// Queues a [`WindowEvent::RedrawRequested`] event to be emitted that aligns with the windowing
    /// system drawing loop.
    ///
    /// This is the **strongly encouraged** method of redrawing windows, as it can integrate with
    /// OS-requested redraws (e.g. when a window gets resized). To improve the event delivery
    /// consider using [`Window::pre_present_notify`] as described in docs.
    ///
    /// Applications should always aim to redraw whenever they receive a `RedrawRequested` event.
    ///
    /// There are no strong guarantees about when exactly a `RedrawRequest` event will be emitted
    /// with respect to other events, since the requirements can vary significantly between
    /// windowing systems.
    ///
    /// However as the event aligns with the windowing system drawing loop, it may not arrive in
    /// same or even next event loop iteration.
    ///
    /// ## Platform-specific
    ///
    /// - **Windows** This API uses `RedrawWindow` to request a `WM_PAINT` message and
    ///   `RedrawRequested` is emitted in sync with any `WM_PAINT` messages.
    /// - **Wayland:** The events are aligned with the frame callbacks when
    ///   [`Window::pre_present_notify`] is used.
    /// - **Web:** [`WindowEvent::RedrawRequested`] will be aligned with the
    ///   `requestAnimationFrame`.
    ///
    /// [`WindowEvent::RedrawRequested`]: crate::event::WindowEvent::RedrawRequested
    fn request_redraw(&self);

    /// Notify the windowing system before presenting to the window.
    ///
    /// You should call this event after your drawing operations, but before you submit
    /// the buffer to the display or commit your drawings. Doing so will help winit to properly
    /// schedule and make assumptions about its internal state. For example, it could properly
    /// throttle [`WindowEvent::RedrawRequested`].
    ///
    /// ## Example
    ///
    /// This example illustrates how it looks with OpenGL, but it applies to other graphics
    /// APIs and software rendering.
    ///
    /// ```no_run
    /// # use winit_core::window::Window;
    /// # fn swap_buffers() {}
    /// # fn scope(window: &dyn Window) {
    /// // Do the actual drawing with OpenGL.
    ///
    /// // Notify winit that we're about to submit buffer to the windowing system.
    /// window.pre_present_notify();
    ///
    /// // Submit buffer to the windowing system.
    /// swap_buffers();
    /// # }
    /// ```
    ///
    /// ## Platform-specific
    ///
    /// - **Android / iOS / X11 / Web / Windows / macOS / Orbital:** Unsupported.
    /// - **Wayland:** Schedules a frame callback to throttle [`WindowEvent::RedrawRequested`].
    ///
    /// [`WindowEvent::RedrawRequested`]: crate::event::WindowEvent::RedrawRequested
    fn pre_present_notify(&self);

    /// Returns the size of the window's render-able surface.
    ///
    /// This is the dimensions you should pass to things like Wgpu or Glutin when configuring the
    /// surface for drawing. See [`WindowEvent::SurfaceResized`] for listening to changes to this
    /// field.
    ///
    /// Note that to ensure that your content is not obscured by things such as notches or the title
    /// bar, you will likely want to only draw important content inside a specific area of the
    /// surface, see [`safe_area()`] for details.
    ///
    /// ## Platform-specific
    ///
    /// - **Web:** Returns the size of the canvas element. Doesn't account for CSS [`transform`].
    ///
    /// [`transform`]: https://developer.mozilla.org/en-US/docs/Web/CSS/transform
    /// [`WindowEvent::SurfaceResized`]: crate::event::WindowEvent::SurfaceResized
    /// [`safe_area()`]: Window::safe_area
    fn surface_size(&self) -> PhysicalSize<u32>;

    /// Request the new size for the surface.
    ///
    /// On platforms where the size is entirely controlled by the user the
    /// applied size will be returned immediately, resize event in such case
    /// may not be generated.
    ///
    /// On platforms where resizing is disallowed by the windowing system, the current surface size
    /// is returned immediately, and the user one is ignored.
    ///
    /// When `None` is returned, it means that the request went to the display system,
    /// and the actual size will be delivered later with the [`WindowEvent::SurfaceResized`].
    ///
    /// See [`Window::surface_size`] for more information about the values.
    ///
    /// The request could automatically un-maximize the window if it's maximized.
    ///
    /// ```no_run
    /// # use dpi::{LogicalSize, PhysicalSize};
    /// # use winit_core::window::Window;
    /// # fn scope(window: &dyn Window) {
    /// // Specify the size in logical dimensions like this:
    /// let _ = window.request_surface_size(LogicalSize::new(400.0, 200.0).into());
    ///
    /// // Or specify the size in physical dimensions like this:
    /// let _ = window.request_surface_size(PhysicalSize::new(400, 200).into());
    /// # }
    /// ```
    ///
    /// ## Platform-specific
    ///
    /// - **Web:** Sets the size of the canvas element. Doesn't account for CSS [`trkansform`].
    ///
    /// [`WindowEvent::SurfaceResized`]: crate::event::WindowEvent::SurfaceResized
    /// [`transform`]: https://developer.mozilla.org/en-US/docs/Web/CSS/transform
    #[must_use]
    fn request_surface_size(&self, size: Size) -> Option<PhysicalSize<u32>>;

    /// Change the window transparency state.
    ///
    /// This is just a hint that may not change anything about
    /// the window transparency, however doing a mismatch between
    /// the content of your window and this hint may result in
    /// visual artifacts.
    ///
    /// The default value follows the [`WindowAttributes::with_transparent`].
    ///
    /// ## Platform-specific
    ///
    /// - **macOS:** This will reset the window's background color.
    /// - **Web / iOS / Android:** Unsupported.
    /// - **X11:** Can only be set while building the window, with
    ///   [`WindowAttributes::with_transparent`].
    fn set_transparent(&self, transparent: bool);
}

/// Identifier of a window. Unique for each window.
///
/// Can be obtained with [`window.id()`][`Window::id`].
///
/// Whenever you receive an event specific to a window, this event contains a `WindowId` which you
/// can then compare to the ids of your windows.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SurfaceId(usize);

impl SurfaceId {
    /// Convert the `WindowId` into the underlying integer.
    ///
    /// This is useful if you need to pass the ID across an FFI boundary, or store it in an atomic.
    pub const fn into_raw(self) -> usize {
        self.0
    }

    /// Construct a `WindowId` from the underlying integer.
    ///
    /// This should only be called with integers returned from [`WindowId::into_raw`].
    pub const fn from_raw(id: usize) -> Self {
        Self(id)
    }
}

impl fmt::Debug for SurfaceId {
    fn fmt(&self, fmtr: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(fmtr)
    }
}