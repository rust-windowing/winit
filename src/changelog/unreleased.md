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

When the change requires non-trivial amount of work for users to comply
with it, the migration guide should be added below the entry, like:

```md
- Deprecate `Window` creation outside of `EventLoop::run`

  This was done to simply migration in the future. Consider the
  following code:

  // Code snippet.

  To migrate it we should do X, Y, and then Z, for example:

  // Code snippet.

```

The migration guide could reference other migration examples in the current
changelog entry.

## Unreleased

### Added

- Add `ActiveEventLoop::create_proxy()`.
- On Web, add `ActiveEventLoopExtWeb::is_cursor_lock_raw()` to determine if
  `DeviceEvent::MouseMotion` is returning raw data, not OS accelerated, when using
  `CursorGrabMode::Locked`.
- On Web, implement `MonitorHandle` and `VideoModeHandle`.

  Without prompting the user for permission, only the current monitor is returned. But when
  prompting and being granted permission through
  `ActiveEventLoop::request_detailed_monitor_permission()`, access to all monitors and their
  details is available. Handles created with "detailed monitor permissions" can be used in
  `Window::set_fullscreen()` as well.

  Keep in mind that handles do not auto-upgrade after permissions are granted and have to be
  re-created to make full use of this feature.
- Add `Touch::finger_id` with a new type `FingerId`.
- On Web and Windows, add `FingerIdExt*::is_primary()`, exposing a way to determine
  the primary finger in a multi-touch interaction.
- Implement `Clone`, `Copy`, `Debug`, `Deserialize`, `Eq`, `Hash`, `Ord`, `PartialEq`, `PartialOrd`
  and `Serialize` on many types.
- Add `MonitorHandle::current_video_mode()`.
- Add basic iOS IME support. The soft keyboard can now be shown using `Window::set_ime_allowed`.
- On macOS, add `WindowExtMacOS::set_borderless_game` and `WindowAttributesExtMacOS::with_borderless_game`
  to fully disable the menu bar and dock in Borderless Fullscreen as commonly done in games.

### Changed

- Change `ActiveEventLoop` to be a trait.
- Change `Window` to be a trait.
- `ActiveEventLoop::create_window` now returns `Box<dyn Window>`.
- `ApplicationHandler` now uses `dyn ActiveEventLoop`.
- On Web, let events wake up event loop immediately when using `ControlFlow::Poll`.
- Bump MSRV from `1.70` to `1.73`.
- Changed `ApplicationHandler::user_event` to `user_wake_up`, removing the
  generic user event.

  Winit will now only indicate that wake up happened, you will have to pair
  this with an external mechanism like `std::sync::mpsc::channel` if you want
  to send specific data to be processed on the main thread.
- Changed `EventLoopProxy::send_event` to `EventLoopProxy::wake_up`, it now
  only wakes up the loop.
- On X11, implement smooth resizing through the sync extension API.
- `ApplicationHandler::create|destroy_surfaces()` was split off from
  `ApplicationHandler::resumed/suspended()`.

  `ApplicationHandler::can_create_surfaces()` should, for portability reasons
  to Android, be the only place to create render surfaces.

  `ApplicationHandler::resumed/suspended()` are now only emitted by iOS and Web
  and now signify actually resuming/suspending the application.
- Rename `platform::web::*ExtWebSys` to `*ExtWeb`.
- Change signature of `EventLoop::run_app`, `EventLoopExtPumpEvents::pump_app_events` and
  `EventLoopExtRunOnDemand::run_app_on_demand` to accept a `impl ApplicationHandler` directly,
  instead of requiring a `&mut` reference to it.
- On Web, `Window::canvas()` now returns a reference.
- On Web, `CursorGrabMode::Locked` now lets `DeviceEvent::MouseMotion` return raw data, not OS
  accelerated, if the browser supports it.
- `(Active)EventLoop::create_custom_cursor()` now returns a `Result<CustomCursor, ExternalError>`.
- Changed how `ModifiersState` is serialized by Serde.
- `VideoModeHandle::refresh_rate_millihertz()` and `bit_depth()` now return a `Option<NonZero*>`.
- `MonitorHandle::position()` now returns an `Option`.
- On iOS and macOS, remove custom application delegates. You are now allowed to override the
  application delegate yourself.
- On iOS, no longer act as-if the application successfully open all URLs. Override
  `application:didFinishLaunchingWithOptions:` and provide the desired behaviour yourself.
- On X11, remove our dependency on libXcursor. (#3749)
- Renamed the following APIs to make it clearer that the sizes apply to the underlying surface:
  - `WindowEvent::Resized` to `SurfaceResized`.
  - `InnerSizeWriter` to `SurfaceSizeWriter`.
  - `WindowAttributes.inner_size` to `surface_size`.
  - `WindowAttributes.min_inner_size` to `min_surface_size`.
  - `WindowAttributes.max_inner_size` to `max_surface_size`.
  - `WindowAttributes.resize_increments` to `surface_resize_increments`.
  - `WindowAttributes::with_inner_size` to `with_surface_size`.
  - `WindowAttributes::with_min_inner_size` to `with_min_surface_size`.
  - `WindowAttributes::with_max_inner_size` to `with_max_surface_size`.
  - `WindowAttributes::with_resize_increments` to `with_surface_resize_increments`.
  - `Window::inner_size` to `surface_size`.
  - `Window::request_inner_size` to `request_surface_size`.
  - `Window::set_min_inner_size` to `set_min_surface_size`.
  - `Window::set_max_inner_size` to `set_max_surface_size`.

  To migrate, you can probably just replace all instances of `inner_size` with `surface_size` in your codebase.

### Removed

- Remove `Event`.
- Remove already deprecated APIs:
  - `EventLoop::create_window()`
  - `EventLoop::run`.
  - `EventLoopBuilder::new()`
  - `EventLoopExtPumpEvents::pump_events`.
  - `EventLoopExtRunOnDemand::run_on_demand`.
  - `VideoMode`
  - `WindowAttributes::new()`
  - `Window::set_cursor_icon()`
- On iOS, remove `platform::ios::EventLoopExtIOS` and related `platform::ios::Idiom` type.

  This feature was incomplete, and the equivalent functionality can be trivially achieved outside
  of `winit` using `objc2-ui-kit` and calling `UIDevice::currentDevice().userInterfaceIdiom()`.
- On Web, remove unused `platform::web::CustomCursorError::Animation`.
- Remove the `rwh_04` and `rwh_05` cargo feature and the corresponding `raw-window-handle` v0.4 and
  v0.5 support. v0.6 remains in place and is enabled by default.
- Remove `DeviceEvent::Added` and `DeviceEvent::Removed`.
- Remove `DeviceEvent::Motion` and `WindowEvent::AxisMotion`.
- Remove `Touch::id` in favor of `Touch::finger_id`.
- Remove `MonitorHandle::size()` and `refresh_rate_millihertz()` in favor of
  `MonitorHandle::current_video_mode()`.
- On Android, remove all `MonitorHandle` support instead of emitting false data.

### Fixed

- On Orbital, `MonitorHandle::name()` now returns `None` instead of a dummy name.
- On macOS, fix `WindowEvent::Moved` sometimes being triggered unnecessarily on resize.
- On MacOS, package manifest definitions of `LSUIElement` will no longer be overridden with the
  default activation policy, unless explicitly provided during initialization.
