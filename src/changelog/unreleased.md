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

### Changed

- On Wayland, bump `smithay-client-toolkit` to 0.20, `sctk-adwaita` to 0.11, and `calloop` to 0.14. This moves `sctk-adwaita` onto `tiny-skia` 0.12, unblocking downstream crates (e.g. egui) from upgrading `resvg` past 0.45.
- Bump MSRV from 1.70 to 1.71, since `smithay-client-toolkit` 0.20 depends on `thiserror` 2.0 which requires Rust 1.71.
