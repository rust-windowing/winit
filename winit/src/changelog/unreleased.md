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
- Implement `Send` and `Sync` for `OwnedDisplayHandle`.
- Use new macOS 15 cursors for resize icons.
- On Android, added scancode conversions for more obscure key codes.

### Changed

- Updated `windows-sys` to `v0.61`.
- On older macOS versions (tested up to 12.7.6), applications now receive mouse movement events for unfocused windows, matching the behavior on other platforms.

### Fixed

- On macOS, synthesize per-key `KeyboardInput` press/release events on focus changes, matching X11 and Windows. Previously only `ModifiersChanged` was emitted on focus loss, and nothing on focus gain, leaving per-key modifier tracking stale.
- On Redox, handle `EINTR` when reading from `event_socket` instead of panicking.
- On Wayland, switch from using the `ahash` hashing algorithm to `foldhash`.
- On macOS, fix borderless game presentation options not sticking after switching spaces.
- On macOS, fix IME being locked on (regardless of requests to disable) after being enabled once.
