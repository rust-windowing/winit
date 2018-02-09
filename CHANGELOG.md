# Unreleased

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
