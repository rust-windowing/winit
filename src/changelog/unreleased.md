The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

The sections should follow the order `Added`, `Changed`, `Deprecated`,
`Removed`, and `Fixed`.

Platform specific changed should be added to the end of the section and grouped
by platform name. Common API additions should have `, implemented` at the end
for platforms where the API was initially implemented. See the following example
on how to add them:

```md
### Added

- Add `Window::turbo()`, implemented on X11, Wayland, and Web.
- On X11, add `Window::some_rare_api`.
- On X11, add `Window::even_more_rare_api`.
- On Wayland, add `Window::common_api`.
- On Windows, add `Window::some_rare_api`.
```

## Unreleased

### Added

- Add `OwnedDisplayHandle` type for allowing safe display handle usage outside of
  trivial cases.
- Add `ApplicationHandler<T>` trait which mimics `Event<T>`.
- Add `WindowBuilder::with_cursor` and `Window::set_cursor` which takes a
  Add `CursorIcon` or `CustomCursor`.
- Add `Sync` implementation for `EventLoopProxy<T: Send>`.
- Add `Window::default_attributes` to get default `WindowAttributes`.
- Add `EventLoop::builder` to get `EventLoopBuilder` without export.
- Add `CustomCursor::from_rgba` to allow creating cursor images from RGBA data.
- Add `CustomCursorExtWebSys::from_url` to allow loading cursor images from URLs.
- Add `CustomCursorExtWebSys::from_animation` to allow creating animated cursors
  Add from other `CustomCursor`s.
- Add `{Active,}EventLoop::create_custom_cursor` to load custom cursor image sources.
- Add `ActiveEventLoop::create_window` and `EventLoop::create_window`.
- Add `CustomCursor` which could be set via `Window::set_cursor`, implemented on
  Windows, macOS, X11, Wayland, and Web.
- On Web, add to toggle calling `Event.preventDefault()` on `Window`.
- On iOS, add `PinchGesture`, `DoubleTapGesture`, and `RotationGesture`
- On macOS, add services menu.
- On Windows, add `with_title_text_color`, and `with_corner_preference` on
  `WindowAttributesExtWindows`.

### Changed

- Bump MSRV from `1.65` to `1.70`.
- Rename `TouchpadMagnify` to `PinchGesture`.
- Rename `SmartMagnify` to `DoubleTapGesture`.
- Rename `TouchpadRotate` to `RotationGesture`.
- Rename `EventLoopWindowTarget` to `ActiveEventLoop`.
- Rename `platform::x11::XWindowType` to `platform::x11::WindowType`.
- Rename `VideoMode` to `VideoModeHandle` to represent that it doesn't hold
  static data.
- Move `dpi` types to its own crate, and re-export it from the root crate.
- Replace `log` with `tracing`, use `log` feature on `tracing` to restore old
  behavior.
- `EventLoop::with_user_event` now returns `EventLoopBuilder`.
- On Web, return `HandleError::Unavailable` when a window handle is not available.
- On Web, return `RawWindowHandle::WebCanvas` instead of `RawWindowHandle::Web`.
- On Web, remove queuing fullscreen request in absence of transient activation.
- On iOS, return `HandleError::Unavailable` when a window handle is not available.
- On macOS, return `HandleError::Unavailable` when a window handle is not available.
- On Windows, remove `WS_CAPTION`, `WS_BORDER`, and `WS_EX_WINDOWEDGE` styles
  for child windows without decorations.

### Deprecated

- Deprecate `EventLoop::run`, use `EventLoop::run_app`.
- Deprecate `EventLoopExtRunOnDemand::run_on_demand`, use `EventLoop::run_app_on_demand`.
- Deprecate `EventLoopExtPumpEvents::pump_events`, use `EventLoopExtPumpEvents::pump_app_events`.
- Deprecate `Window::set_cursor_icon`, use `Window::set_cursor`.

### Removed

- Remove `Window::new`, use `ActiveEventLoop::create_window` and `EventLoop::create_window`.
- Remove `Deref` implementation for `EventLoop` that gave `EventLoopWindowTarget`.
- Remove `WindowBuilder` in favor of `WindowAttributes`.
- Remove Generic parameter `T` from `ActiveEventLoop`.
- Remove `EventLoopBuilder::with_user_event`, use `EventLoop::with_user_event`.
- Remove Redundant `EventLoopError::AlreadyRunning`.
- Remove `WindowAttributes::fullscreen` and expose as field directly.
- On X11, remove `platform::x11::XNotSupported` export.

### Fixed

- On Web, fix setting cursor icon overriding cursor visibility.
- On Windows, fix cursor not confined to center of window when grabbed and hidden.
- On macOS, fix sequence of mouse events being out of order when dragging on the trackpad.
