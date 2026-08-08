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

- Add `keyboard` support for OpenHarmony.
- On iOS, add Apple Pencil support with force, altitude, and azimuth data.
- On Redox, add support for missing keyboard scancodes.
- On Redox, add support for `EventLoopExtPumpEvents::pump_app_events`.
- Implement `Send` and `Sync` for `OwnedDisplayHandle`.
- Use new macOS 15 cursors for resize icons.
- On Android, added scancode conversions for more obscure key codes.
- On Wayland, added `HoldGesture` event for multi-finger hold gestures
- On Wayland, added ext-background-effect-v1 support.
- On Wayland, Windows and macOS, added native popups (`WindowType::Popup`).
- On macOS, add `WindowAttributesMacOS::with_fullscreen_auxiliary` and
  `WindowExtMacOS::set_fullscreen_auxiliary` / `WindowExtMacOS::fullscreen_auxiliary`, allowing a
  window to be shown on the same Space as a fullscreen window
  (`NSWindowCollectionBehaviorFullScreenAuxiliary`) instead of triggering a Space switch or Split
  View tiling.
- Add `WindowEvent::PointerButton::is_macos_activation_click`. On macOS, both the press and
  matching release of a click that activated a previously inactive window are tagged, so
  applications can ignore activation clicks for buttons or destructive actions while accepting
  them for low-risk actions like selection or scrolling. Always `false` on other platforms.
- `winit::event_loop::EventLoopProvider` trait with common event loop methods.

### Changed

- Mark the extensible public enums as `#[non_exhaustive]`: `StartCause`, `WindowEvent`,
  `DeviceEvent`, `Ime`, `PointerKind`, `PointerSource`, `ButtonSource`, `NativeKey`,
  `NativeKeyCode`, `CustomCursorSource`, `TypeHint`, `SendData`, `DndAction`, `ImeRequest`,
  `BadIcon`, `BadImage`, `BadAnimation`, `ImeSurroundingTextError`, `MouseScrollDelta`,
  and `Fullscreen`.

  When matching on one of these types, add a wildcard arm to cover variants added in the future:

  ```rust,ignore
  match event {
      WindowEvent::CloseRequested => (),
      // ...
      _ => (),
  }
  ```
- On macOS, mark `ActivationPolicy` as `#[non_exhaustive]`.
- On X11, mark `WindowType` and `UriListParseError` as `#[non_exhaustive]`.
- On Web, mark `PollStrategy`, `WaitUntilStrategy`, `CustomCursorError`, `MonitorPermissionError`,
  and `OrientationLockError` as `#[non_exhaustive]`.
- On Windows, mark `BackdropType` and `CornerPreference` as `#[non_exhaustive]`.
- On iOS, mark `ValidOrientations` and `StatusBarStyle` as `#[non_exhaustive]`.
- Updated `windows-sys` to `v0.61`.
- On older macOS versions (tested up to 12.7.6), applications now receive mouse movement events for unfocused windows, matching the behavior on other platforms.
- On macOS, using the private API `CGSSetWindowBackgroundBlurRadius` for `Window::set_blur` is now disabled by default. It can be re-enabled using the Cargo feature `private-apple-apis`.

### Removed

- On macOS, remove `WindowAttributesMacOS::with_accepts_first_mouse`. Use the new per-event
  `WindowEvent::PointerButton::is_macos_activation_click` flag instead. To preserve the old
  `with_accepts_first_mouse(false)` behavior, ignore `PointerButton` press events (and their
  matching releases / drags) where `is_macos_activation_click` is `true`.

### Fixed

- On Windows, fix a freeze that occurs when the keyboard layout is switched by
  tools such as Punto Switcher. The `WM_INPUTLANGCHANGE` message is now handled
  to refresh the cached keyboard layout, while still deferring to
  `DefWindowProc` for normal propagation.
- On Windows, fix getting the window's DPI internally leaks `HDC` handles.
  Also only call `GetDC` when on < Windows 8.1 which improves its performance.
- On Redox, handle `EINTR` when reading from `event_socket` instead of panicking.
- On Wayland, switch from using the `ahash` hashing algorithm to `foldhash`.
- On macOS, fix borderless game presentation options not sticking after switching spaces.
- On macOS, fix IME being locked on (regardless of requests to disable) after being enabled once.
- On macOS, fix a panic and incorrect cursor position in Ime::Preedit when the preedit string contains special characters (ie. emojis) caused by incorrect UTF-16 to UTF-8 offset conversion.
- On Wayland, fix a protocol error when setting a custom cursor on compositors with `wl_surface` version below 3.
- On Redox, fix `run_app_on_demand` exiting immediately after a previous `run_app_on_demand` called `exit`.
- On Redox, fill in logical key for keyboard events rather than emitting a separate fake IME event.
- On Redox, handle window closes during `ApplicationHandler` drop.
