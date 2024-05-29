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
- Add traits `EventLoopExtWayland` and `EventLoopExtX11`, providing methods `is_wayland` and `is_x11` on `EventLoop`.
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

- Reexport `raw-window-handle` versions 0.4 and 0.5 as `raw_window_handle_04` and `raw_window_handle_05`.
- Implement `ApplicationHandler` for `&mut` references and heap allocations to something that implements `ApplicationHandler`.

### Removed

- Remove `EventLoop::run`.
- Remove `EventLoopExtRunOnDemand::run_on_demand`.
- Remove `EventLoopExtPumpEvents::pump_events`.

### Fixed

- On macOS, fix panic on exit when dropping windows outside the event loop.
- On macOS, fix window dragging glitches when dragging across a monitor boundary with different scale factor.
