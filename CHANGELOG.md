# Unreleased

- `Icon::to_cardinals` is no longer public, since it was never supposed to be.
- Wayland: improve diagnostics if initialization fails
- Fix some system event key doesn't work when focused, do not block keyevent forward to system on macOS
- On X11, `Window::get_current_monitor` now reliably returns the correct monitor.
- On X11, `Window::hidpi_factor` returns values from XRandR rather than the inaccurate values previously queried from the core protocol.
- On X11, the primary monitor is detected correctly even when using versions of XRandR less than 1.5.
- `MonitorId` now implements `Debug`.

# Version 0.14.0 (2018-05-09)

- Created the `Copy`, `Paste` and `Cut` `VirtualKeyCode`s and added support for them on X11 and Wayland
- Fix `.with_decorations(false)` in macOS
- On Mac, `NSWindow` and supporting objects might be alive long after they were `closed` which resulted in apps consuming more heap then needed. Mainly it was affecting multi window applications. Not expecting any user visible change of behaviour after the fix.
- Fix regression of Window platform extensions for macOS where `NSFullSizeContentViewWindowMask` was not being correctly applied to `.fullsize_content_view`.
- Corrected `get_position` on Windows to be relative to the screen rather than to the taskbar.
- Corrected `Moved` event on Windows to use position values equivalent to those returned by `get_position`. It previously supplied client area positions instead of window positions, and would additionally interpret negative values as being very large (around `u16::MAX`).
- Implemented `Moved` event on macOS.
- On X11, the `Moved` event correctly use window positions rather than client area positions. Additionally, a stray `Moved` that unconditionally accompanied `Resized` with the client area position relative to the parent has been eliminated; `Moved` is still received alongside `Resized`, but now only once and always correctly.
- On Windows, implemented all variants of `DeviceEvent` other than `Text`. Mouse `DeviceEvent`s are now received even if the window isn't in the foreground.
- `DeviceId` on Windows is no longer a unit struct, and now contains a `u32`. For `WindowEvent`s, this will always be 0, but on `DeviceEvent`s it will be the handle to that device. `DeviceIdExt::get_persistent_identifier` can be used to acquire a unique identifier for that device that persists across replugs/reboots/etc.
- Corrected `run_forever` on X11 to stop discarding `Awakened` events.
- Various safety and correctness improvements to the X11 backend internals.
- Fixed memory leak on X11 every time the mouse entered the window.
- On X11, drag and drop now works reliably in release mode.
- Added `WindowBuilderExt::with_resize_increments` and `WindowBuilderExt::with_base_size` to X11, allowing for more optional hints to be set.
- Rework of the wayland backend, migrating it to use [Smithay's Client Toolkit](https://github.com/Smithay/client-toolkit).
- Added `WindowBuilder::with_window_icon` and `Window::set_window_icon`, finally making it possible to set the window icon on Windows and X11. The `icon_loading` feature can be enabled to allow for icons to be easily loaded; see example program `window_icon.rs` for usage.
- Windows additionally has `WindowBuilderExt::with_taskbar_icon` and `WindowExt::set_taskbar_icon`.
- On Windows, fix panic when trying to call `set_fullscreen(None)` on a window that has not been fullscreened prior.

# Version 0.13.1 (2018-04-26)

- Ensure necessary `x11-dl` version is used.

# Version 0.13.0 (2018-04-25)

- Implement `WindowBuilder::with_maximized`, `Window::set_fullscreen`, `Window::set_maximized` and `Window::set_decorations` for MacOS.
- Implement `WindowBuilder::with_maximized`, `Window::set_fullscreen`, `Window::set_maximized` and `Window::set_decorations` for Windows.
- On Windows, `WindowBuilder::with_fullscreen` no longer changing monitor display resolution.
- Overhauled X11 window geometry calculations. `get_position` and `set_position` are more universally accurate across different window managers, and `get_outer_size` actually works now.
- Fixed SIGSEGV/SIGILL crashes on macOS caused by stabilization of the `!` (never) type.
- Implement `WindowEvent::HiDPIFactorChanged` for macOS
- On X11, input methods now work completely out of the box, no longer requiring application developers to manually call `setlocale`. Additionally, when input methods are started, stopped, or restarted on the server end, it's correctly handled.
- Implemented `Refresh` event on Windows.
- Properly calculate the minimum and maximum window size on Windows, including window decorations.
- Map more `MouseCursor` variants to cursor icons on Windows.
- Corrected `get_position` on macOS to return outer frame position, not content area position.
- Corrected `set_position` on macOS to set outer frame position, not content area position.
- Added `get_inner_position` method to `Window`, which gets the position of the window's client area. This is implemented on all applicable platforms (all desktop platforms other than Wayland, where this isn't possible).
- **Breaking:** the `Closed` event has been replaced by `CloseRequested` and `Destroyed`. To migrate, you typically just need to replace all usages of `Closed` with `CloseRequested`; see example programs for more info. The exception is iOS, where `Closed` must be replaced by `Destroyed`.

# Version 0.12.0 (2018-04-06)

- Added subclass to macos windows so they can be made resizable even with no decorations.
- Dead keys now work properly on X11, no longer resulting in a panic.
- On X11, input method creation first tries to use the value from the user's `XMODIFIERS` environment variable, so application developers should no longer need to manually call `XSetLocaleModifiers`. If that fails, fallbacks are tried, which should prevent input method initialization from ever outright failing.
- Fixed thread safety issues with input methods on X11.
- Add support for `Touch` for win32 backend.
- Fixed `Window::get_inner_size` and friends to return the size in pixels instead of points when using HIDPI displays on OSX.

# Version 0.11.3 (2018-03-28)

- Added `set_min_dimensions` and `set_max_dimensions` methods to `Window`, and implemented on Windows, X11, Wayland, and OSX.
- On X11, dropping a `Window` actually closes it now, and clicking the window's × button (or otherwise having the WM signal to close it) will result in the window closing.
- Added `WindowBuilderExt` methods for macos: `with_titlebar_transparent`,
  `with_title_hidden`, `with_titlebar_buttons_hidden`,
  `with_fullsize_content_view`.
- Mapped X11 numpad keycodes (arrows, Home, End, PageUp, PageDown, Insert and Delete) to corresponding virtual keycodes

# Version 0.11.2 (2018-03-06)

- Impl `Hash`, `PartialEq`, and `Eq` for `events::ModifiersState`.
- Implement `MonitorId::get_hidpi_factor` for MacOS.
- Added method `os::macos::MonitorIdExt::get_nsscreen() -> *mut c_void` that gets a `NSScreen` object matching the monitor ID.
- Send `Awakened` event on Android when event loop is woken up.

# Version 0.11.1 (2018-02-19)

- Fixed windows not receiving mouse events when click-dragging the mouse outside the client area of a window, on Windows platforms.
- Added method `os::android::EventsLoopExt:set_suspend_callback(Option<Box<Fn(bool) -> ()>>)` that allows glutin to register a callback when a suspend event happens

# Version 0.11.0 (2018-02-09)

- Implement `MonitorId::get_dimensions` for Android.
- Added method `os::macos::WindowBuilderExt::with_movable_by_window_background(bool)` that allows to move a window without a titlebar - `with_decorations(false)`
- Implement `Window::set_fullscreen`, `Window::set_maximized` and `Window::set_decorations` for Wayland.
- Added `Caret` as VirtualKeyCode and support OSX ^-Key with german input.

# Version 0.10.1 (2018-02-05)

*Yanked*

# Version 0.10.0 (2017-12-27)

- Add support for `Touch` for emscripten backend.
- Added support for `DroppedFile`, `HoveredFile`, and `HoveredFileCancelled` to X11 backend.
- **Breaking:** `unix::WindowExt` no longer returns pointers for things that aren't actually pointers; `get_xlib_window` now returns `Option<std::os::raw::c_ulong>` and `get_xlib_screen_id` returns `Option<std::os::raw::c_int>`. Additionally, methods that previously returned `libc::c_void` have been changed to return `std::os::raw::c_void`, which are not interchangeable types, so users wanting the former will need to explicitly cast.
- Added `set_decorations` method to `Window` to allow decorations to be toggled after the window is built. Presently only implemented on X11.
- Raised the minimum supported version of Rust to 1.20 on MacOS due to usage of associated constants in new versions of cocoa and core-graphics.
- Added `modifiers` field to `MouseInput`, `MouseWheel`, and `CursorMoved` events to track the modifiers state (`ModifiersState`).
- Fixed the emscripten backend to return the size of the canvas instead of the size of the window.

# Version 0.9.0 (2017-12-01)

- Added event `WindowEvent::HiDPIFactorChanged`.
- Added method `MonitorId::get_hidpi_factor`.
- Deprecated `get_inner_size_pixels` and `get_inner_size_points` methods of `Window` in favor of
`get_inner_size`.
- **Breaking:** `EventsLoop` is `!Send` and `!Sync` because of platform-dependant constraints,
  but `Window`, `WindowId`, `DeviceId` and `MonitorId` guaranteed to be `Send`.
- `MonitorId::get_position` now returns `(i32, i32)` instead of `(u32, u32)`.
- Rewrite of the wayland backend to use wayland-client-0.11
- Support for dead keys on wayland for keyboard utf8 input
- Monitor enumeration on Windows is now implemented using `EnumDisplayMonitors` instead of
`EnumDisplayDevices`. This changes the value returned by `MonitorId::get_name()`.
- On Windows added `MonitorIdExt::hmonitor` method
- Impl `Clone` for `EventsLoopProxy`
- `EventsLoop::get_primary_monitor()` on X11 will fallback to any available monitor if no primary is found
- Support for touch event on wayland
- `WindowEvent`s `MouseMoved`, `MouseEntered`, and `MouseLeft` have been renamed to
`CursorMoved`, `CursorEntered`, and `CursorLeft`.
- New `DeviceEvent`s added, `MouseMotion` and `MouseWheel`.
- Send `CursorMoved` event after `CursorEntered` and `Focused` events.
- Add support for `ModifiersState`, `MouseMove`, `MouseInput`, `MouseMotion` for emscripten backend.

# Version 0.8.3 (2017-10-11)

- Fixed issue of calls to `set_inner_size` blocking on Windows.
- Mapped `ISO_Left_Tab` to `VirtualKeyCode::Tab` to make the key work with modifiers
- Fixed the X11 backed on 32bit targets

# Version 0.8.2 (2017-09-28)

- Uniformize keyboard scancode values accross Wayland and X11 (#297).
- Internal rework of the wayland event loop
- Added method `os::linux::WindowExt::is_ready`

# Version 0.8.1 (2017-09-22)

- Added various methods to `os::linux::EventsLoopExt`, plus some hidden items necessary to make
  glutin work.

# Version 0.8.0 (2017-09-21)

- Added `Window::set_maximized`, `WindowAttributes::maximized` and `WindowBuilder::with_maximized`.
- Added `Window::set_fullscreen`.
- Changed `with_fullscreen` to take a `Option<MonitorId>` instead of a `MonitorId`.
- Removed `MonitorId::get_native_identifer()` in favor of platform-specific traits in the `os`
  module.
- Changed `get_available_monitors()` and `get_primary_monitor()` to be methods of `EventsLoop`
  instead of stand-alone methods.
- Changed `EventsLoop` to be tied to a specific X11 or Wayland connection.
- Added a `os::linux::EventsLoopExt` trait that makes it possible to configure the connection.
- Fixed the emscripten code, which now compiles.
- Changed the X11 fullscreen code to use `xrandr` instead of `xxf86vm`.
- Fixed the Wayland backend to produce `Refresh` event after window creation.
- Changed the `Suspended` event to be outside of `WindowEvent`.
- Fixed the X11 backend sometimes reporting the wrong virtual key (#273).
