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

- On Windows, add `IconExtWindows::from_resource_name`.
- On Windows, add `CursorGrabMode::Locked`.
- On Wayland, add `WindowExtWayland::xdg_toplevel`.

### Changed

- On macOS, no longer need control of the main `NSApplication` class (which means you can now override it yourself).
- On iOS, remove custom application delegates. You are now allowed to override the
  application delegate yourself.
- On iOS, no longer act as-if the application successfully open all URLs. Override
  `application:didFinishLaunchingWithOptions:` and provide the desired behaviour yourself.

### Fixed

- On Windows, fixed ~500 ms pause when clicking the title bar during continuous redraw.
- On macos, `WindowExtMacOS::set_simple_fullscreen` now honors `WindowExtMacOS::set_borderless_game`
- On X11 and Wayland, fixed pump_events with `Some(Duration::Zero)` blocking with `Wait` polling mode
- On Wayland, fixed a crash when consequently calling `set_cursor_grab` without pointer focus.
- On Wayland, ensure that external event loop is woken-up when using pump_events and integrating via `FD`.
- On Wayland, apply fractional scaling to custom cursors.
- On macOS, fixed `run_app_on_demand` returning without closing open windows.
- On macOS, fixed `VideoMode::refresh_rate_millihertz` for fractional refresh rates.
