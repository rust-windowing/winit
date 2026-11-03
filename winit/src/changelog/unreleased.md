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

- Add `WindowEvent::KeysHeldAtFocus` and `HeldKey`, reporting the keys held when
  a window gains focus, implemented on Windows, X11, Wayland, and macOS.

### Changed

- On Windows, `ModifiersChanged` sent when gaining focus now follows
  `Focused(true)`.

### Removed

- Remove `is_synthetic` from `WindowEvent::KeyboardInput`. On Windows and X11,
  key presses are no longer sent for the keys held when a window gains focus,
  nor key releases for the keys held when it loses focus.

  Applications that did not check `is_synthetic` acted on these presses as if
  the keys had just been pressed. Patterns binding `is_synthetic` must remove
  it. To know which keys are held when a window gains focus, match on
  `WindowEvent::KeysHeldAtFocus` instead. When a window loses focus, treat every
  key as released.

### Fixed

- On macOS, fix text selected from the Character Viewer not producing an IME commit.
- On macOS, fix the release of a modifier key held when the window gains focus
  not being reported.
- On macOS, fix modifiers held when a window gains focus not being reported
  until they change.
- On Wayland, fix text not being copied to some application due to missing some mime types.
