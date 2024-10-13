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

- On macOS, add `WindowExtMacOS::set_borderless_game` and `WindowAttributesExtMacOS::with_borderless_game`
  to fully disable the menu bar and dock in Borderless Fullscreen as commonly done in games.

### Fixed

- On macOS, fix `WindowEvent::Moved` sometimes being triggered unnecessarily on resize.
- On macOS, package manifest definitions of `LSUIElement` will no longer be overridden with the
  default activation policy, unless explicitly provided during initialization.
- On macOS, fix crash when calling `drag_window()` without a left click present.
- On X11, key events forward to IME anyway, even when it's disabled.
- On Windows, make `ControlFlow::WaitUntil` work more precisely using `CREATE_WAITABLE_TIMER_HIGH_RESOLUTION`.
