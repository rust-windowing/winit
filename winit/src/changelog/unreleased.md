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

### Changed

- Updated `windows-sys` to `v0.61`.

### Fixed

- On X11, fix `set_hittest` not working on some window managers.
- On Redox, handle `EINTR` when reading from `event_socket` instead of panicking.
- On Wayland, switch from using the `ahash` hashing algorithm to `foldhash`.
- On macOS, fix borderless game presentation options not sticking after switching spaces.
- On macOS, fix crash in `set_marked_text` when native Pinyin IME sends out-of-bounds `selected_range`.
- On X11, fix debug mode overflow panic in `set_timestamp`.
