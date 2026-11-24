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

### Changed

- Bump MSRV from `1.86` to `1.88`.

## Unreleased

### Added

- Add `EventTime` and `WindowEvent::event_time()`, implemented on Android, iOS, macOS, Wayland,
  Web, Windows, and X11.
- Add `HistoricalMoveEvent`, reported in `WindowEvent::PointerMoved::history`.

### Changed

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
- On Wayland, a window created with a parent window now calls `xdg_toplevel.set_parent` with that
  parent, and is closed together with its parent: it receives `WindowEvent::Destroyed` and no
  further events. The window stays visible until the application drops it, and so does its parent,
  which is only destroyed once all its child windows and popups have been dropped.
- On Wayland, creating a window whose parent is not a Wayland window, not a window created by the
  same event loop, or a popup now fails with `CreateWindowError::InvalidAttribute`. Previously,
  the parent was silently ignored.

### Fixed

- Fix `RgbaIcon::new` and `CustomCursorSource::from_rgba` rejecting every valid
  RGBA buffer.
- On macOS, fix text selected from the Character Viewer not producing an IME commit.
- On Wayland, fix text not being copied to some application due to missing some mime types.
