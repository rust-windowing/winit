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
- Add `WindowEvent::CursorMoved::type` with a new type `CursorType` introducing pen/stylus support.

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
- `Force::normalized()` now takes a `Option<ToolAngle>` to calculate the perpendicular force.

### Removed

- Remove `EventLoop::run`.
- Remove `EventLoopExtRunOnDemand::run_on_demand`.
- Remove `EventLoopExtPumpEvents::pump_events`.
- Remove `Event`.
- On iOS, remove `platform::ios::EventLoopExtIOS` and related `platform::ios::Idiom` type.

  This feature was incomplete, and the equivalent functionality can be trivially achieved outside
  of `winit` using `objc2-ui-kit` and calling `UIDevice::currentDevice().userInterfaceIdiom()`.
- On Web, remove unused `platform::web::CustomCursorError::Animation`.
- Remove `Force::Calibrated::altitude_angle` in favor of `ToolAngle::altitude`.

### Fixed

- On MacOS, fix building with `feature = "rwh_04"`.
