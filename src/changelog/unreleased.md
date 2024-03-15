The sections should follow the order `Added`, `Changed`, `Deprecated`,
`Removed`, and `Fixed`. Platform specific changes should have their own
subsection, like `Wayland`. When API is initially added and implemented on a
multiple platforms, it's indicated with the `New API, implemnted on X11,
Wayland, and macOS`

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## Unreleased

### Added

- `OwnedDisplayHandle` type for allowing safe display handle usage outside of
  trivial cases
- `ApplicationHandler<T>` trait which mimics `Event<T>`
- `WindowBuilder::with_cursor` and `Window::set_cursor` which takes a
  `CursorIcon` or `CustomCursor`
- `Sync` implementation for `EventLoopProxy<T: Send>`
- `Window::default_attributes` to get default `WindowAttributes`
- `EventLoop::builder` to get `EventLoopBuilder` without export
- `CustomCursor::from_rgba` to allow creating cursor images from RGBA data
- `CustomCursorExtWebSys::from_url` to allow loading cursor images from URLs
- `CustomCursorExtWebSys::from_animation` to allow creating animated cursors
  from other `CustomCursor`s
- `{Active,}EventLoop::create_custom_cursor` to load custom cursor image sources
- `ActiveEventLoop::create_window` and `EventLoop::create_window`
- `CustomCursor` which could be set via `Window::set_cursor`, implemented on Windows, macOS, X11, Wayland, and Web

#### iOS

- Detection support for `PinchGesture`, `DoubleTapGesture`, and
  `RotationGesture`

#### macOS

- Services menu

#### Web

- Ability to toggle calling `Event.preventDefault()` on `Window`

#### Windows

- `with_system_backdrop`, `with_border_color`, `with_title_background_color`,
  `with_title_text_color`, and `with_corner_preference` on
  `WindowAttributesExtWindows`

### Changed

- Bump MSRV from `1.65` to `1.70`.
- Renamed `TouchpadMagnify` to `PinchGesture`
- Renamed `SmartMagnify` to `DoubleTapGesture`
- Renamed `TouchpadRotate` to `RotationGesture`
- Renamed `EventLoopWindowTarget` to `ActiveEventLoop`.
- Renamed `platform::x11::XWindowType` to `platform::x11::WindowType`.
- Renamed `VideoMode` to `VideoModeHandle` to represent that it doesn't hold
  static data.
- Moved `dpi` types to its own crate, and re-export it from the root crate
- `log` has been replaced with `tracing`. The old behavior can be emulated by
  setting the `log` feature on the `tracing` crate.
- `EventLoop::with_user_event` now returns `EventLoopBuilder`

#### iOS

- Return `HandleError::Unavailable` when a window handle is not available

#### macOS

- Return `HandleError::Unavailable` when a window handle is not available

#### Web

- Return `HandleError::Unavailable` when a window handle is not available
- Return `RawWindowHandle::WebCanvas` instead of `RawWindowHandle::Web`

#### Windows

- Removed `WS_CAPTION`, `WS_BORDER`, and `WS_EX_WINDOWEDGE` styles for child
  windows without decorations

### Deprecated

- `EventLoop::run`, use `EventLoop::run_app`
- `EventLoopExtRunOnDemand::run_on_demand`, use `EventLoop::run_app_on_demand`
- `EventLoopExtPumpEvents::pump_events`, use `EventLoopExtPumpEvents::pump_app_events`
- `Window::set_cursor_icon`, use `Window::set_cursor`
- `Window::new`, use `ActiveEventLoop::create_window` and `EventLoop::create_window`

### Removed

- `Deref` implementation for `EventLoop` that gave `EventLoopWindowTarget`
- `WindowBuilder` in favor of `WindowAttributes`
- Generic parameter `T` from `ActiveEventLoop`
- `EventLoopBuilder::with_user_event`, use `EventLoop::with_user_event`
- Redundant `EventLoopError::AlreadyRunning`
- `WindowAttributes::fullscreen` and expose as field directly

#### Web

- Queuing fullscreen request in absence of transient activation

#### X11

- `platform::x11::XNotSupported` export

### Fixed

#### Web

- Setting cursor icon overriding cursor visibility

#### Windows

- Confine cursor to center of window when grabbed and hidden
