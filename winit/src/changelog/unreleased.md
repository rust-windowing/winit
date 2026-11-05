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

- Add `EventTime` and `WindowEvent::event_time()`, implemented on Android, iOS, macOS, Wayland,
  Web, Windows, and X11.
- Add `HistoricalMoveEvent`, reported in `WindowEvent::PointerMoved::history`.

### Fixed

- On Windows, fix a crash occurring when trying to create a DXGI surface on a window that is both fullscreen and always on top.
- On Wayland, use the touch serial for `Window::drag_window`, `Window::drag_resize_window` and `Window::show_window_menu` when the interaction was started by touch.
- On Windows, fix a crash when moving a window that owns windows not created by winit, such as native dialogs or application helper windows.
- On Windows, avoid overflow when negating minimum mouse-wheel deltas.
- On X11, reset the drag and drop state when a drag operation ends. A later drag no longer reuses the state of a previous drag.
- On Wayland, fix the text input being destroyed whenever a seat capability changed. This made IME input stop working in already-running applications after the seat's capabilities were updated.
- On Wayland, fix a panic when a popup is dropped before its queued compositor
  updates are processed.

### Changed

- Removed the `url` dependency from `winit-core`, along with `SendData::from_file_paths`,
  `TypedData::try_as_file_paths` and `impl From<Vec<url::Url>> for SendData`. Use `SendData::Uris`
  / `TypedData::try_as_uris` with `file:` URIs instead.
- Add an `event_time: Option<EventTime>` field to the `WindowEvent` variants `KeyboardInput`,
  `PointerMoved`, `PointerEntered`, `PointerLeft`, `MouseWheel`, `PointerButton`, `HoldGesture`,
  `PinchGesture`, `PanGesture`, `DoubleTapGesture`, `RotationGesture` and `TouchpadPressure`. It
  holds the time at which the input occurred. The time has no fixed origin, so it can only be
  compared with other event times from the same event loop.
  To get the time of any event without matching on it, use `WindowEvent::event_time()`.
- Add a `history: Vec<HistoricalMoveEvent>` field to `WindowEvent::PointerMoved`. It holds the
  intermediate samples that the platform coalesced into the event, ordered from oldest to newest.
  It doesn't include the event's own sample. It is filled on Android, iOS and Web, and is empty on
  other platforms.
- On Web, coalesced pointer samples from `getCoalescedEvents()` are now reported in
  `WindowEvent::PointerMoved::history`, instead of as separate `WindowEvent::PointerMoved` events.
