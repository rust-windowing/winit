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
- On Web, implement `Error` for `platform::web::CustomCursorError`.
- On Web, add `ActiveEventLoopExtWeb::is_cursor_lock_raw()` to determine if
  `DeviceEvent::MouseMotion` is returning raw data, not OS accelerated, when using
  `CursorGrabMode::Locked`.
- On Web, implement `MonitorHandle` and `VideoModeHandle`.
  
  Without prompting the user for permission, only the current monitor is returned. But when
  prompting and being granted permission through
  `ActiveEventLoop::request_detailed_monitor_permission()`, access to all monitors and their
  information is available. This "detailed monitors" can be used in `Window::set_fullscreen()` as
  well.
- On Android, add `{Active,}EventLoopExtAndroid::android_app()` to access the app used to create the loop.

### Changed

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

### Fixed

- On Web, pen events are now routed through to `WindowEvent::Cursor*`.
