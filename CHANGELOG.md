# Changelog

All notable changes to this project will be documented in this file.

Please keep one empty line before and after all headers. (This is required for `git` to produce a conflict when a release is made while a PR is open and the PR's changelog entry would go into the wrong section).

And please only add new entries to the top of this list, right below the `# Unreleased` header.

# Unreleased

- **Breaking:** Split the `platform::unix` module into `platform::x11` and `platform::wayland`. The extension types are similarly renamed.
- **Breaking:**: Removed deprecated method `platform::unix::WindowExtUnix::is_ready`.
- Removed `parking_lot` dependency.
- On Windows, added `WindowExtWindows::set_undecorated_shadow` and `WindowBuilderExtWindows::with_undecorated_shadow` to draw the drop shadow behind a borderless window.
- On Windows, fixed default window features (ie snap, animations, shake, etc.) when decorations are disabled.
- **Breaking:** On macOS, add support for two-finger touchpad magnification and rotation gestures with new events `WindowEvent::TouchpadMagnify` and `WindowEvent::TouchpadRotate`.
- On Wayland, `wayland-csd-adwaita` now uses `ab_glyph` instead of `crossfont` to render the title for decorations.
- On Wayland, a new `wayland-csd-adwaita-crossfont` feature was added to use `crossfont` instead of `ab_glyph` for decorations.
- On Wayland, if not otherwise specified use upstream automatic CSD theme selection.
- On Windows, fixed ALT+Space shortcut to open window menu.

# 0.27.2 (2022-8-12)

- On macOS, fixed touch phase reporting when scrolling.
- On X11, fix min, max and resize increment hints not persisting for resizable windows (e.g. on DPI change).
- On Windows, respect min/max inner sizes when creating the window.
- For backwards compatibility, `Window` now (additionally) implements the old version (`0.4`) of the `HasRawWindowHandle` trait
- On Windows, added support for `EventLoopWindowTarget::set_device_event_filter`.
- On Wayland, fix user requested `WindowEvent::RedrawRequested` being delayed by a frame.

# 0.27.1 (2022-07-30)

- The minimum supported Rust version was lowered to `1.57.0` and now explicitly tested.
- On X11, fix crash on start due to inability to create an IME context without any preedit.

# 0.27.0 (2022-07-26)

- On Windows, fix hiding a maximized window.
- On Android, `ndk-glue`'s `NativeWindow` lock is now held between `Event::Resumed` and `Event::Suspended`.
- On Web, added `EventLoopExtWebSys` with a `spawn` method to start the event loop without throwing an exception.
- Added `WindowEvent::Occluded(bool)`, currently implemented on macOS and X11.
- On X11, fix events for caps lock key not being sent
- Build docs on `docs.rs` for iOS and Android as well.
- **Breaking:** Removed the `WindowAttributes` struct, since all its functionality is accessible from `WindowBuilder`.
- Added `WindowBuilder::transparent` getter to check if the user set `transparent` attribute.
- On macOS, Fix emitting `Event::LoopDestroyed` on CMD+Q.
- On macOS, fixed an issue where having multiple windows would prevent run_return from ever returning.
- On Wayland, fix bug where the cursor wouldn't hide in GNOME.
- On macOS, Windows, and Wayland, add `set_cursor_hittest` to let the window ignore mouse events.
- On Windows, added `WindowExtWindows::set_skip_taskbar` and `WindowBuilderExtWindows::with_skip_taskbar`.
- On Windows, added `EventLoopBuilderExtWindows::with_msg_hook`.
- On Windows, remove internally unique DC per window.
- On macOS, remove the need to call `set_ime_position` after moving the window.
- Added `Window::is_visible`.
- Added `Window::is_resizable`.
- Added `Window::is_decorated`.
- On X11, fix for repeated event loop iteration when `ControlFlow` was `Wait`
- On X11, fix scale factor calculation when the only monitor is reconnected
- On Wayland, report unaccelerated mouse deltas in `DeviceEvent::MouseMotion`.
- On Web, a focused event is manually generated when a click occurs to emulate behaviour of other backends.
- **Breaking:** Bump `ndk` version to 0.6, ndk-sys to `v0.3`, `ndk-glue` to `0.6`.
- Remove no longer needed `WINIT_LINK_COLORSYNC` environment variable.
- **Breaking:** Rename the `Exit` variant of `ControlFlow` to `ExitWithCode`, which holds a value to control the exit code after running. Add an `Exit` constant which aliases to `ExitWithCode(0)` instead to avoid major breakage. This shouldn't affect most existing programs.
- Add `EventLoopBuilder`, which allows you to create and tweak the settings of an event loop before creating it.
- Deprecated `EventLoop::with_user_event`; use `EventLoopBuilder::with_user_event` instead.
- **Breaking:** Replaced `EventLoopExtMacOS` with `EventLoopBuilderExtMacOS` (which also has renamed methods).
- **Breaking:** Replaced `EventLoopExtWindows` with `EventLoopBuilderExtWindows` (which also has renamed methods).
- **Breaking:** Replaced `EventLoopExtUnix` with `EventLoopBuilderExtUnix` (which also has renamed methods).
- **Breaking:** The platform specific extensions for Windows `winit::platform::windows` have changed. All `HANDLE`-like types e.g. `HWND` and `HMENU` were converted from winapi types or `*mut c_void` to `isize`. This was done to be consistent with the type definitions in windows-sys and to not expose internal dependencies.
- The internal bindings to the [Windows API](https://docs.microsoft.com/en-us/windows/) were changed from the unofficial [winapi](https://github.com/retep998/winapi-rs) bindings to the official Microsoft [windows-sys](https://github.com/microsoft/windows-rs) bindings.
- On Wayland, fix polling during consecutive `EventLoop::run_return` invocations.
- On Windows, fix race issue creating fullscreen windows with `WindowBuilder::with_fullscreen`
- On Android, `virtual_keycode` for `KeyboardInput` events is now filled in where a suitable match is found.
- Added helper methods on `ControlFlow` to set its value.
- On Wayland, fix `TouchPhase::Ended` always reporting the location of the first touch down, unless the compositor
  sent a cancel or frame event.
- On iOS, send `RedrawEventsCleared` even if there are no redraw events, consistent with other platforms.
- **Breaking:** Replaced `Window::with_app_id` and `Window::with_class` with `Window::with_name` on `WindowBuilderExtUnix`.
- On Wayland, fallback CSD was replaced with proper one:
  - `WindowBuilderExtUnix::with_wayland_csd_theme` to set color theme in builder.
  - `WindowExtUnix::wayland_set_csd_theme` to set color theme when creating a window.
  - `WINIT_WAYLAND_CSD_THEME` env variable was added, it can be used to set "dark"/"light" theme in apps that don't expose theme setting.
  - `wayland-csd-adwaita` feature that enables proper CSD with title rendering using FreeType system library.
  - `wayland-csd-adwaita-notitle` feature that enables CSD but without title rendering.
- On Wayland and X11, fix window not resizing with `Window::set_inner_size` after calling `Window:set_resizable(false)`.
- On Windows, fix wrong fullscreen monitors being recognized when handling WM_WINDOWPOSCHANGING messages
- **Breaking:** Added new `WindowEvent::Ime` supported on desktop platforms.
- Added `Window::set_ime_allowed` supported on desktop platforms.
- **Breaking:** IME input on desktop platforms won't be received unless it's explicitly allowed via `Window::set_ime_allowed` and new `WindowEvent::Ime` events are handled.
- On macOS, `WindowEvent::Resized` is now emitted in `frameDidChange` instead of `windowDidResize`.
- **Breaking:** On X11, device events are now ignored for unfocused windows by default, use `EventLoopWindowTarget::set_device_event_filter` to set the filter level.
- Implemented `Default` on `EventLoop<()>`.
- Implemented `Eq` for `Fullscreen`, `Theme`, and `UserAttentionType`.
- **Breaking:** `Window::set_cursor_grab` now accepts `CursorGrabMode` to control grabbing behavior.
- On Wayland, add support for `Window::set_cursor_position`.
- Fix on macOS `WindowBuilder::with_disallow_hidpi`, setting true or false by the user no matter the SO default value.
- `EventLoopBuilder::build` will now panic when the `EventLoop` is being created more than once.
- Added `From<u64>` for `WindowId` and `From<WindowId>` for `u64`.
- Added `MonitorHandle::refresh_rate_millihertz` to get monitor's refresh rate.
- **Breaking**, Replaced `VideoMode::refresh_rate` with `VideoMode::refresh_rate_millihertz` providing better precision.
- On Web, add `with_prevent_default` and `with_focusable` to `WindowBuilderExtWebSys` to control whether events should be propagated.
- On Windows, fix focus events being sent to inactive windows.
- **Breaking**, update `raw-window-handle` to `v0.5` and implement `HasRawDisplayHandle` for `Window` and `EventLoopWindowTarget`.
- On X11, add function `register_xlib_error_hook` into `winit::platform::unix` to subscribe for errors comming from Xlib.
- On Android, upgrade `ndk` and `ndk-glue` dependencies to the recently released `0.7.0`.
- All platforms can now be relied on to emit a `Resumed` event. Applications are recommended to lazily initialize graphics state and windows on first resume for portability.
- **Breaking:**: Reverse horizontal scrolling sign in `MouseScrollDelta` to match the direction of vertical scrolling. A positive X value now means moving the content to the right. The meaning of vertical scrolling stays the same: a positive Y value means moving the content down.

# 0.26.1 (2022-01-05)

- Fix linking to the `ColorSync` framework on macOS 10.7, and in newer Rust versions.
- On Web, implement cursor grabbing through the pointer lock API.
- On X11, add mappings for numpad comma, numpad enter, numlock and pause.
- On macOS, fix Pinyin IME input by reverting a change that intended to improve IME.
- On Windows, fix a crash with transparent windows on Windows 11.

# 0.26.0 (2021-12-01)

- Update `raw-window-handle` to `v0.4`. This is _not_ a breaking change, we still implement `HasRawWindowHandle` from `v0.3`, see [rust-windowing/raw-window-handle#74](https://github.com/rust-windowing/raw-window-handle/pull/74). Note that you might have to run `cargo update -p raw-window-handle` after upgrading.
- On X11, bump `mio` to 0.8.
- On Android, fixed `WindowExtAndroid::config` initially returning an empty `Configuration`.
- On Android, fixed `Window::scale_factor` and `MonitorHandle::scale_factor` initially always returning 1.0.
- On X11, select an appropriate visual for transparency if is requested
- On Wayland and X11, fix diagonal window resize cursor orientation.
- On macOS, drop the event callback before exiting.
- On Android, implement `Window::request_redraw`
- **Breaking:** On Web, remove the `stdweb` backend.
- Added `Window::focus_window`to bring the window to the front and set input focus.
- On Wayland and X11, implement `is_maximized` method on `Window`.
- On Windows, prevent ghost window from showing up in the taskbar after either several hours of use or restarting `explorer.exe`.
- On macOS, fix issue where `ReceivedCharacter` was not being emitted during some key repeat events.
- On Wayland, load cursor icons `hand2` and `hand1` for `CursorIcon::Hand`.
- **Breaking:** On Wayland, Theme trait and its support types are dropped.
- On Wayland, bump `smithay-client-toolkit` to 0.15.1.
- On Wayland, implement `request_user_attention` with `xdg_activation_v1`.
- On X11, emit missing `WindowEvent::ScaleFactorChanged` when the only monitor gets reconnected.
- On X11, if RANDR based scale factor is higher than 20 reset it to 1
- On Wayland, add an enabled-by-default feature called `wayland-dlopen` so users can opt out of using `dlopen` to load system libraries.
- **Breaking:** On Android, bump `ndk` and `ndk-glue` to 0.5.
- On Windows, increase wait timer resolution for more accurate timing when using `WaitUntil`.
- On macOS, fix native file dialogs hanging the event loop.
- On Wayland, implement a workaround for wrong configure size when using `xdg_decoration` in `kwin_wayland`
- On macOS, fix an issue that prevented the menu bar from showing in borderless fullscreen mode.
- On X11, EINTR while polling for events no longer causes a panic. Instead it will be treated as a spurious wakeup.

# 0.25.0 (2021-05-15)

- **Breaking:** On macOS, replace `WindowBuilderExtMacOS::with_activation_policy` with `EventLoopExtMacOS::set_activation_policy`
- On macOS, wait with activating the application until the application has initialized.
- On macOS, fix creating new windows when the application has a main menu.
- On Windows, fix fractional deltas for mouse wheel device events.
- On macOS, fix segmentation fault after dropping the main window.
- On Android, `InputEvent::KeyEvent` is partially implemented providing the key scancode.
- Added `is_maximized` method to `Window`.
- On Windows, fix bug where clicking the decoration bar would make the cursor blink.
- On Windows, fix bug causing newly created windows to erroneously display the "wait" (spinning) cursor.
- On macOS, wake up the event loop immediately when a redraw is requested.
- On Windows, change the default window size (1024x768) to match the default on other desktop platforms (800x600).
- On Windows, fix bug causing mouse capture to not be released.
- On Windows, fix fullscreen not preserving minimized/maximized state.
- On Android, unimplemented events are marked as unhandled on the native event loop.
- On Windows, added `WindowBuilderExtWindows::with_menu` to set a custom menu at window creation time.
- On Android, bump `ndk` and `ndk-glue` to 0.3: use predefined constants for event `ident`.
- On macOS, fix objects captured by the event loop closure not being dropped on panic.
- On Windows, fixed `WindowEvent::ThemeChanged` not properly firing and fixed `Window::theme` returning the wrong theme.
- On Web, added support for `DeviceEvent::MouseMotion` to listen for relative mouse movements.
- Added `WindowBuilder::with_position` to allow setting the position of a `Window` on creation. Supported on Windows, macOS and X11.
- Added `Window::drag_window`. Implemented on Windows, macOS, X11 and Wayland.
- On X11, bump `mio` to 0.7.
- On Windows, added `WindowBuilderExtWindows::with_owner_window` to allow creating popup windows.
- On Windows, added `WindowExtWindows::set_enable` to allow creating modal popup windows.
- On macOS, emit `RedrawRequested` events immediately while the window is being resized.
- Implement `Default`, `Hash`, and `Eq` for `LogicalPosition`, `PhysicalPosition`, `LogicalSize`, and `PhysicalSize`.
- On macOS, initialize the Menu Bar with minimal defaults. (Can be prevented using `enable_default_menu_creation`)
- On macOS, change the default behavior for first click when the window was unfocused. Now the window becomes focused and then emits a `MouseInput` event on a "first mouse click".
- Implement mint (math interoperability standard types) conversions (under feature flag `mint`).

# 0.24.0 (2020-12-09)

- On Windows, fix applications not exiting gracefully due to thread_event_target_callback accessing corrupted memory.
- On Windows, implement `Window::set_ime_position`.
- **Breaking:** On Windows, Renamed `WindowBuilderExtWindows`'s `is_dark_mode` to `theme`.
- **Breaking:** On Windows, renamed `WindowBuilderExtWindows::is_dark_mode` to `theme`.
- On Windows, add `WindowBuilderExtWindows::with_theme` to set a preferred theme.
- On Windows, fix bug causing message boxes to appear delayed.
- On Android, calling `WindowEvent::Focused` now works properly instead of always returning false.
- On Windows, fix Alt-Tab behaviour by removing borderless fullscreen "always on top" flag.
- On Windows, fix bug preventing windows with transparency enabled from having fully-opaque regions.
- **Breaking:** On Windows, include prefix byte in scancodes.
- On Wayland, fix window not being resizeable when using `WindowBuilder::with_min_inner_size`.
- On Unix, fix cross-compiling to wasm32 without enabling X11 or Wayland.
- On Windows, fix use-after-free crash during window destruction.
- On Web, fix `WindowEvent::ReceivedCharacter` never being sent on key input.
- On macOS, fix compilation when targeting aarch64.
- On X11, fix `Window::request_redraw` not waking the event loop.
- On Wayland, the keypad arrow keys are now recognized.
- **Breaking** Rename `desktop::EventLoopExtDesktop` to `run_return::EventLoopExtRunReturn`.
- Added `request_user_attention` method to `Window`.
- **Breaking:** On macOS, removed `WindowExt::request_user_attention`, use `Window::request_user_attention`.
- **Breaking:** On X11, removed `WindowExt::set_urgent`, use `Window::request_user_attention`.
- On Wayland, default font size in CSD increased from 11 to 17.
- On Windows, fix bug causing message boxes to appear delayed.
- On Android, support multi-touch.
- On Wayland, extra mouse buttons are not dropped anymore.
- **Breaking**: `MouseButton::Other` now uses `u16`.

# 0.23.0 (2020-10-02)

- On iOS, fixed support for the "Debug View Heirarchy" feature in Xcode.
- On all platforms, `available_monitors` and `primary_monitor` are now on `EventLoopWindowTarget` rather than `EventLoop` to list monitors event in the event loop.
- On Unix, X11 and Wayland are now optional features (enabled by default)
- On X11, fix deadlock when calling `set_fullscreen_inner`.
- On Web, prevent the webpage from scrolling when the user is focused on a winit canvas
- On Web, calling `window.set_cursor_icon` no longer breaks HiDPI scaling
- On Windows, drag and drop is now optional (enabled by default) and can be disabled with `WindowBuilderExtWindows::with_drag_and_drop(false)`.
- On Wayland, fix deadlock when calling to `set_inner_size` from a callback.
- On macOS, add `hide__other_applications` to `EventLoopWindowTarget` via existing `EventLoopWindowTargetExtMacOS` trait. `hide_other_applications` will hide other applications by calling `-[NSApplication hideOtherApplications: nil]`.
- On android added support for `run_return`.
- On MacOS, Fixed fullscreen and dialog support for `run_return`.
- On Windows, fix bug where we'd try to emit `MainEventsCleared` events during nested win32 event loops.
- On Web, use mouse events if pointer events aren't supported. This affects Safari.
- On Windows, `set_ime_position` is now a no-op instead of a runtime crash.
- On Android, `set_fullscreen` is now a no-op instead of a runtime crash.
- On iOS and Android, `set_inner_size` is now a no-op instead of a runtime crash.
- On Android, fix `ControlFlow::Poll` not polling the Android event queue.
- On macOS, add `NSWindow.hasShadow` support.
- On Web, fix vertical mouse wheel scrolling being inverted.
- On Web, implement mouse capturing for click-dragging out of the canvas.
- On Web, fix `ControlFlow::Exit` not properly handled.
- On Web (web-sys only), send `WindowEvent::ScaleFactorChanged` event when `window.devicePixelRatio` is changed.
- **Breaking:** On Web, `set_cursor_position` and `set_cursor_grab` will now always return an error.
- **Breaking:** `PixelDelta` scroll events now return a `PhysicalPosition`.
- On NetBSD, fixed crash due to incorrect detection of the main thread.
- **Breaking:** On X11, `-` key is mapped to the `Minus` virtual key code, instead of `Subtract`.
- On macOS, fix inverted horizontal scroll.
- **Breaking:** `current_monitor` now returns `Option<MonitorHandle>`.
- **Breaking:** `primary_monitor` now returns `Option<MonitorHandle>`.
- On macOS, updated core-* dependencies and cocoa.
- Bump `parking_lot` to 0.11
- On Android, bump `ndk`, `ndk-sys` and `ndk-glue` to 0.2. Checkout the new ndk-glue main proc attribute.
- On iOS, fixed starting the app in landscape where the view still had portrait dimensions.
- Deprecate the stdweb backend, to be removed in a future release
- **Breaking:** Prefixed virtual key codes `Add`, `Multiply`, `Divide`, `Decimal`, and `Subtract` with `Numpad`.
- Added `Asterisk` and `Plus` virtual key codes.
- On Web (web-sys only), the `Event::LoopDestroyed` event is correctly emitted when leaving the page.
- On Web, the `WindowEvent::Destroyed` event now gets emitted when a `Window` is dropped.
- On Web (web-sys only), the event listeners are now removed when a `Window` is dropped or when the event loop is destroyed.
- On Web, the event handler closure passed to `EventLoop::run` now gets dropped after the event loop is destroyed.
- **Breaking:** On Web, the canvas element associated to a `Window` is no longer removed from the DOM when the `Window` is dropped.
- On Web, `WindowEvent::Resized` is now emitted when `Window::set_inner_size` is called.
- **Breaking:** `Fullscreen` enum now uses `Borderless(Option<MonitorHandle>)` instead of `Borderless(MonitorHandle)` to allow picking the current monitor.
- On MacOS, fix `WindowEvent::Moved` ignoring the scale factor.
- On Wayland, add missing virtual keycodes.
- On Wayland, implement proper `set_cursor_grab`.
- On Wayland, the cursor will use similar icons if the requested one isn't available.
- On Wayland, right clicking on client side decorations will request application menu.
- On Wayland, fix tracking of window size after state changes.
- On Wayland, fix client side decorations not being hidden properly in fullscreen.
- On Wayland, fix incorrect size event when entering fullscreen with client side decorations.
- On Wayland, fix `resizable` attribute not being applied properly on startup.
- On Wayland, fix disabled repeat rate not being handled.
- On Wayland, fix decoration buttons not working after tty switch.
- On Wayland, fix scaling not being applied on output re-enable.
- On Wayland, fix crash when `XCURSOR_SIZE` is `0`.
- On Wayland, fix pointer getting created in some cases without pointer capability.
- On Wayland, on kwin, fix space between window and decorations on startup.
- **Breaking:** On Wayland, `Theme` trait was reworked.
- On Wayland, disable maximize button for non-resizable window.
- On Wayland, added support for `set_ime_position`.
- On Wayland, fix crash on startup since GNOME 3.37.90.
- On X11, fix incorrect modifiers state on startup.

# 0.22.2 (2020-05-16)

- Added Clone implementation for 'static events.
- On Windows, fix window intermittently hanging when `ControlFlow` was set to `Poll`.
- On Windows, fix `WindowBuilder::with_maximized` being ignored.
- On Android, minimal platform support.
- On iOS, touch positions are now properly converted to physical pixels.
- On macOS, updated core-* dependencies and cocoa

# 0.22.1 (2020-04-16)

- On X11, fix `ResumeTimeReached` being fired too early.
- On Web, replaced zero timeout for `ControlFlow::Poll` with `requestAnimationFrame`
- On Web, fix a possible panic during event handling
- On macOS, fix `EventLoopProxy` leaking memory for every instance.

# 0.22.0 (2020-03-09)

- On Windows, fix minor timing issue in wait_until_time_or_msg
- On Windows, rework handling of request_redraw() to address panics.
- On macOS, fix `set_simple_screen` to remember frame excluding title bar.
- On Wayland, fix coordinates in touch events when scale factor isn't 1.
- On Wayland, fix color from `close_button_icon_color` not applying.
- Ignore locale if unsupported by X11 backend
- On Wayland, Add HiDPI cursor support
- On Web, add the ability to query "Light" or "Dark" system theme send `ThemeChanged` on change.
- Fix `Event::to_static` returning `None` for user events.
- On Wayland, Hide CSD for fullscreen windows.
- On Windows, ignore spurious mouse move messages.
- **Breaking:** Move `ModifiersChanged` variant from `DeviceEvent` to `WindowEvent`.
- On Windows, add `IconExtWindows` trait which exposes creating an `Icon` from an external file or embedded resource
- Add `BadIcon::OsError` variant for when OS icon functionality fails
- On Windows, fix crash at startup on systems that do not properly support Windows' Dark Mode
- Revert On macOS, fix not sending ReceivedCharacter event for specific keys combinations.
- on macOS, fix incorrect ReceivedCharacter events for some key combinations.
- **Breaking:** Use `i32` instead of `u32` for position type in `WindowEvent::Moved`.
- On macOS, a mouse motion event is now generated before every mouse click.

# 0.21.0 (2020-02-04)

- On Windows, fixed "error: linking with `link.exe` failed: exit code: 1120" error on older versions of windows.
- On macOS, fix set_minimized(true) works only with decorations.
- On macOS, add `hide_application` to `EventLoopWindowTarget` via a new `EventLoopWindowTargetExtMacOS` trait. `hide_application` will hide the entire application by calling `-[NSApplication hide: nil]`.
- On macOS, fix not sending ReceivedCharacter event for specific keys combinations.
- On macOS, fix `CursorMoved` event reporting the cursor position using logical coordinates.
- On macOS, fix issue where unbundled applications would sometimes open without being focused.
- On macOS, fix `run_return` does not return unless it receives a message.
- On Windows, fix bug where `RedrawRequested` would only get emitted every other iteration of the event loop.
- On X11, fix deadlock on window state when handling certain window events.
- `WindowBuilder` now implements `Default`.
- **Breaking:** `WindowEvent::CursorMoved` changed to `f64` units, preserving high-precision data supplied by most backends
- On Wayland, fix coordinates in mouse events when scale factor isn't 1
- On Web, add the ability to provide a custom canvas
- **Breaking:** On Wayland, the `WaylandTheme` struct has been replaced with a `Theme` trait, allowing for extra configuration

# 0.20.0 (2020-01-05)

- On X11, fix `ModifiersChanged` emitting incorrect modifier change events
- **Breaking**: Overhaul how Winit handles DPI:
  - Window functions and events now return `PhysicalSize` instead of `LogicalSize`.
  - Functions that take `Size` or `Position` types can now take either `Logical` or `Physical` types.
  - `hidpi_factor` has been renamed to `scale_factor`.
  - `HiDpiFactorChanged` has been renamed to `ScaleFactorChanged`, and lets you control how the OS
    resizes the window in response to the change.
  - On X11, deprecate `WINIT_HIDPI_FACTOR` environment variable in favor of `WINIT_X11_SCALE_FACTOR`.
  - `Size` and `Position` types are now generic over their exact pixel type.

# 0.20.0 Alpha 6 (2020-01-03)

- On macOS, fix `set_cursor_visible` hides cursor outside of window.
- On macOS, fix `CursorEntered` and `CursorLeft` events fired at old window size.
- On macOS, fix error when `set_fullscreen` is called during fullscreen transition.
- On all platforms except mobile and WASM, implement `Window::set_minimized`.
- On X11, fix `CursorEntered` event being generated for non-winit windows.
- On macOS, fix crash when starting maximized without decorations.
- On macOS, fix application not terminating on `run_return`.
- On Wayland, fix cursor icon updates on window borders when using CSD.
- On Wayland, under mutter(GNOME Wayland), fix CSD being behind the status bar, when starting window in maximized mode.
- On Windows, theme the title bar according to whether the system theme is "Light" or "Dark".
- Added `WindowEvent::ThemeChanged` variant to handle changes to the system theme. Currently only implemented on Windows.
- **Breaking**: Changes to the `RedrawRequested` event (#1041):
  - `RedrawRequested` has been moved from `WindowEvent` to `Event`.
  - `EventsCleared` has been renamed to `MainEventsCleared`.
  - `RedrawRequested` is now issued only after `MainEventsCleared`.
  - `RedrawEventsCleared` is issued after each set of `RedrawRequested` events.
- Implement synthetic window focus key events on Windows.
- **Breaking**: Change `ModifiersState` to a `bitflags` struct.
- On Windows, implement `VirtualKeyCode` translation for `LWin` and `RWin`.
- On Windows, fix closing the last opened window causing `DeviceEvent`s to stop getting emitted.
- On Windows, fix `Window::set_visible` not setting internal flags correctly. This resulted in some weird behavior.
- Add `DeviceEvent::ModifiersChanged`.
  - Deprecate `modifiers` fields in other events in favor of `ModifiersChanged`.
- On X11, `WINIT_HIDPI_FACTOR` now dominates `Xft.dpi` when picking DPI factor for output.
- On X11, add special value `randr` for `WINIT_HIDPI_FACTOR` to make winit use self computed DPI factor instead of the one from `Xft.dpi`.

# 0.20.0 Alpha 5 (2019-12-09)

- On macOS, fix application termination on `ControlFlow::Exit`
- On Windows, fix missing `ReceivedCharacter` events when Alt is held.
- On macOS, stop emitting private corporate characters in `ReceivedCharacter` events.
- On X11, fix misreporting DPI factor at startup.
- On X11, fix events not being reported when using `run_return`.
- On X11, fix key modifiers being incorrectly reported.
- On X11, fix window creation hanging when another window is fullscreen.
- On Windows, fix focusing unfocused windows when switching from fullscreen to windowed.
- On X11, fix reporting incorrect DPI factor when waking from suspend.
- Change `EventLoopClosed` to contain the original event.
- **Breaking**: Add `is_synthetic` field to `WindowEvent` variant `KeyboardInput`,
  indicating that the event is generated by winit.
- On X11, generate synthetic key events for keys held when a window gains or loses focus.
- On X11, issue a `CursorMoved` event when a `Touch` event occurs,
  as X11 implicitly moves the cursor for such events.

# 0.20.0 Alpha 4 (2019-10-18)

- Add web support via the 'stdweb' or 'web-sys' features
- On Windows, implemented function to get HINSTANCE
- On macOS, implement `run_return`.
- On iOS, fix inverted parameter in `set_prefers_home_indicator_hidden`.
- On X11, performance is improved when rapidly calling `Window::set_cursor_icon`.
- On iOS, fix improper `msg_send` usage that was UB and/or would break if `!` is stabilized.
- On Windows, unset `maximized` when manually changing the window's position or size.
- On Windows, add touch pressure information for touch events.
- On macOS, differentiate between `CursorIcon::Grab` and `CursorIcon::Grabbing`.
- On Wayland, fix event processing sometimes stalling when using OpenGL with vsync.
- Officially remove the Emscripten backend.
- On Windows, fix handling of surrogate pairs when dispatching `ReceivedCharacter`.
- On macOS 10.15, fix freeze upon exiting exclusive fullscreen mode.
- On iOS, fix panic upon closing the app.
- On X11, allow setting mulitple `XWindowType`s.
- On iOS, fix null window on initial `HiDpiFactorChanged` event.
- On Windows, fix fullscreen window shrinking upon getting restored to a normal window.
- On macOS, fix events not being emitted during modal loops, such as when windows are being resized
  by the user.
- On Windows, fix hovering the mouse over the active window creating an endless stream of CursorMoved events.
- Always dispatch a `RedrawRequested` event after creating a new window.
- On X11, return dummy monitor data to avoid panicking when no monitors exist.
- On X11, prevent stealing input focus when creating a new window.
  Only steal input focus when entering fullscreen mode.
- On Wayland, fixed DeviceEvents for relative mouse movement is not always produced
- On Wayland, add support for set_cursor_visible and set_cursor_grab.
- On Wayland, fixed DeviceEvents for relative mouse movement is not always produced.
- Removed `derivative` crate dependency.
- On Wayland, add support for set_cursor_icon.
- Use `impl Iterator<Item = MonitorHandle>` instead of `AvailableMonitorsIter` consistently.
- On macOS, fix fullscreen state being updated after entering fullscreen instead of before,
  resulting in `Window::fullscreen` returning the old state in `Resized` events instead of
  reflecting the new fullscreen state
- On X11, fix use-after-free during window creation
- On Windows, disable monitor change keyboard shortcut while in exclusive fullscreen.
- On Windows, ensure that changing a borderless fullscreen window's monitor via keyboard shortcuts keeps the window fullscreen on the new monitor.
- Prevent `EventLoop::new` and `EventLoop::with_user_event` from getting called outside the main thread.
  - This is because some platforms cannot run the event loop outside the main thread. Preventing this
    reduces the potential for cross-platform compatibility gotchyas.
- On Windows and Linux X11/Wayland, add platform-specific functions for creating an `EventLoop` outside the main thread.
- On Wayland, drop resize events identical to the current window size.
- On Windows, fix window rectangle not getting set correctly on high-DPI systems.

# 0.20.0 Alpha 3 (2019-08-14)

- On macOS, drop the run closure on exit.
- On Windows, location of `WindowEvent::Touch` are window client coordinates instead of screen coordinates.
- On X11, fix delayed events after window redraw.
- On macOS, add `WindowBuilderExt::with_disallow_hidpi` to have the option to turn off best resolution openGL surface.
- On Windows, screen saver won't start if the window is in fullscreen mode.
- Change all occurrences of the `new_user_event` method to `with_user_event`.
- On macOS, the dock and the menu bar are now hidden in fullscreen mode.
- `Window::set_fullscreen` now takes `Option<Fullscreen>` where `Fullscreen`
  consists of `Fullscreen::Exclusive(VideoMode)` and
  `Fullscreen::Borderless(MonitorHandle)` variants.
  - Adds support for exclusive fullscreen mode.
- On iOS, add support for hiding the home indicator.
- On iOS, add support for deferring system gestures.
- On iOS, fix a crash that occurred while acquiring a monitor's name.
- On iOS, fix armv7-apple-ios compile target.
- Removed the `T: Clone` requirement from the `Clone` impl of `EventLoopProxy<T>`.
- On iOS, disable overscan compensation for external displays (removes black
  bars surrounding the image).
- On Linux, the functions `is_wayland`, `is_x11`, `xlib_xconnection` and `wayland_display` have been moved to a new `EventLoopWindowTargetExtUnix` trait.
- On iOS, add `set_prefers_status_bar_hidden` extension function instead of
  hijacking `set_decorations` for this purpose.
- On macOS and iOS, corrected the auto trait impls of `EventLoopProxy`.
- On iOS, add touch pressure information for touch events.
- Implement `raw_window_handle::HasRawWindowHandle` for `Window` type on all supported platforms.
- On macOS, fix the signature of `-[NSView drawRect:]`.
- On iOS, fix the behavior of `ControlFlow::Poll`. It wasn't polling if that was the only mode ever used by the application.
- On iOS, fix DPI sent out by views on creation was `0.0` - now it gives a reasonable number.
- On iOS, RedrawRequested now works for gl/metal backed views.
- On iOS, RedrawRequested is generally ordered after EventsCleared.

# 0.20.0 Alpha 2 (2019-07-09)

- On X11, non-resizable windows now have maximize explicitly disabled.
- On Windows, support paths longer than MAX_PATH (260 characters) in `WindowEvent::DroppedFile`
and `WindowEvent::HoveredFile`.
- On Mac, implement `DeviceEvent::Button`.
- Change `Event::Suspended(true / false)` to `Event::Suspended` and `Event::Resumed`.
- On X11, fix sanity check which checks that a monitor's reported width and height (in millimeters) are non-zero when calculating the DPI factor.
- Revert the use of invisible surfaces in Wayland, which introduced graphical glitches with OpenGL (#835)
- On X11, implement `_NET_WM_PING` to allow desktop environment to kill unresponsive programs.
- On Windows, when a window is initially invisible, it won't take focus from the existing visible windows.
- On Windows, fix multiple calls to `request_redraw` during `EventsCleared` sending multiple `RedrawRequested events.`
- On Windows, fix edge case where `RedrawRequested` could be dispatched before input events in event loop iteration.
- On Windows, fix timing issue that could cause events to be improperly dispatched after `RedrawRequested` but before `EventsCleared`.
- On macOS, drop unused Metal dependency.
- On Windows, fix the trail effect happening on transparent decorated windows. Borderless (or un-decorated) windows were not affected.
- On Windows, fix `with_maximized` not properly setting window size to entire window.
- On macOS, change `WindowExtMacOS::request_user_attention()` to take an `enum` instead of a `bool`.

# 0.20.0 Alpha 1 (2019-06-21)

- Changes below are considered **breaking**.
- Change all occurrences of `EventsLoop` to `EventLoop`.
- Previously flat API is now exposed through `event`, `event_loop`, `monitor`, and `window` modules.
- `os` module changes:
  - Renamed to `platform`.
  - All traits now have platform-specific suffixes.
  - Exposes new `desktop` module on Windows, Mac, and Linux.
- Changes to event loop types:
  - `EventLoopProxy::wakeup` has been removed in favor of `send_event`.
  - **Major:** New `run` method drives winit event loop.
    - Returns `!` to ensure API behaves identically across all supported platforms.
      - This allows `emscripten` implementation to work without lying about the API.
    - `ControlFlow`'s variants have been replaced with `Wait`, `WaitUntil(Instant)`, `Poll`, and `Exit`.
      - Is read after `EventsCleared` is processed.
      - `Wait` waits until new events are available.
      - `WaitUntil` waits until either new events are available or the provided time has been reached.
      - `Poll` instantly resumes the event loop.
      - `Exit` aborts the event loop.
    - Takes a closure that implements `'static + FnMut(Event<T>, &EventLoop<T>, &mut ControlFlow)`.
      - `&EventLoop<T>` is provided to allow new `Window`s to be created.
  - **Major:** `platform::desktop` module exposes `EventLoopExtDesktop` trait with `run_return` method.
    - Behaves identically to `run`, but returns control flow to the calling context and can take non-`'static` closures.
  - `EventLoop`'s `poll_events` and `run_forever` methods have been removed in favor of `run` and `run_return`.
- Changes to events:
  - Remove `Event::Awakened` in favor of `Event::UserEvent(T)`.
    - Can be sent with `EventLoopProxy::send_event`.
  - Rename `WindowEvent::Refresh` to `WindowEvent::RedrawRequested`.
    - `RedrawRequested` can be sent by the user with the `Window::request_redraw` method.
  - `EventLoop`, `EventLoopProxy`, and `Event` are now generic over `T`, for use in `UserEvent`.
  - **Major:** Add `NewEvents(StartCause)`, `EventsCleared`, and `LoopDestroyed` variants to `Event`.
    - `NewEvents` is emitted when new events are ready to be processed by event loop.
      - `StartCause` describes why new events are available, with `ResumeTimeReached`, `Poll`, `WaitCancelled`, and `Init` (sent once at start of loop).
    - `EventsCleared` is emitted when all available events have been processed.
      - Can be used to perform logic that depends on all events being processed (e.g. an iteration of a game loop).
    - `LoopDestroyed` is emitted when the `run` or `run_return` method is about to exit.
- Rename `MonitorId` to `MonitorHandle`.
- Removed `serde` implementations from `ControlFlow`.
- Rename several functions to improve both internal consistency and compliance with Rust API guidelines.
- Remove `WindowBuilder::multitouch` field, since it was only implemented on a few platforms. Multitouch is always enabled now.
- **Breaking:** On macOS, change `ns` identifiers to use snake_case for consistency with iOS's `ui` identifiers.
- Add `MonitorHandle::video_modes` method for retrieving supported video modes for the given monitor.
- On Wayland, the window now exists even if nothing has been drawn.
- On Windows, fix initial dimensions of a fullscreen window.
- On Windows, Fix transparent borderless windows rendering wrong.

# Version 0.19.1 (2019-04-08)

- On Wayland, added a `get_wayland_display` function to `EventsLoopExt`.
- On Windows, fix `CursorMoved(0, 0)` getting dispatched on window focus.
- On macOS, fix command key event left and right reverse.
- On FreeBSD, NetBSD, and OpenBSD, fix build of X11 backend.
- On Linux, the numpad's add, subtract and divide keys are now mapped to the `Add`, `Subtract` and `Divide` virtual key codes
- On macOS, the numpad's subtract key has been added to the `Subtract` mapping
- On Wayland, the numpad's home, end, page up and page down keys are now mapped to the `Home`, `End`, `PageUp` and `PageDown` virtual key codes
- On Windows, fix icon not showing up in corner of window.
- On X11, change DPI scaling factor behavior. First, winit tries to read it from "Xft.dpi" XResource, and uses DPI calculation from xrandr dimensions as fallback behavior.

# Version 0.19.0 (2019-03-06)

- On X11, we will use the faster `XRRGetScreenResourcesCurrent` function instead of `XRRGetScreenResources` when available.
- On macOS, fix keycodes being incorrect when using a non-US keyboard layout.
- On Wayland, fix `with_title()` not setting the windows title
- On Wayland, add `set_wayland_theme()` to control client decoration color theme
- Added serde serialization to `os::unix::XWindowType`.
- **Breaking:** Remove the `icon_loading` feature and the associated `image` dependency.
- On X11, make event loop thread safe by replacing XNextEvent with select(2) and XCheckIfEvent
- On Windows, fix malformed function pointer typecast that could invoke undefined behavior.
- Refactored Windows state/flag-setting code.
- On Windows, hiding the cursor no longer hides the cursor for all Winit windows - just the one `hide_cursor` was called on.
- On Windows, cursor grabs used to get perpetually canceled when the grabbing window lost focus. Now, cursor grabs automatically get re-initialized when the window regains focus and the mouse moves over the client area.
- On Windows, only vertical mouse wheel events were handled. Now, horizontal mouse wheel events are also handled.
- On Windows, ignore the AltGr key when populating the `ModifersState` type.

# Version 0.18.1 (2018-12-30)

- On macOS, fix `Yen` (JIS) so applications receive the event.
- On X11 with a tiling WM, fixed high CPU usage when moving windows across monitors.
- On X11, fixed panic caused by dropping the window before running the event loop.
- on macOS, added `WindowExt::set_simple_fullscreen` which does not require a separate space
- Introduce `WindowBuilderExt::with_app_id` to allow setting the application ID on Wayland.
- On Windows, catch panics in event loop child thread and forward them to the parent thread. This prevents an invocation of undefined behavior due to unwinding into foreign code.
- On Windows, fix issue where resizing or moving window combined with grabbing the cursor would freeze program.
- On Windows, fix issue where resizing or moving window would eat `Awakened` events.
- On Windows, exiting fullscreen after entering fullscreen with disabled decorations no longer shrinks window.
- On X11, fixed a segfault when using virtual monitors with XRandR.
- Derive `Ord` and `PartialOrd` for `VirtualKeyCode` enum.
- On Windows, fix issue where hovering or dropping a non file item would create a panic.
- On Wayland, fix resizing and DPI calculation when a `wl_output` is removed without sending a `leave` event to the `wl_surface`, such as disconnecting a monitor from a laptop.
- On Wayland, DPI calculation is handled by smithay-client-toolkit.
- On X11, `WindowBuilder::with_min_dimensions` and `WindowBuilder::with_max_dimensions` now correctly account for DPI.
- Added support for generating dummy `DeviceId`s and `WindowId`s to better support unit testing.
- On macOS, fixed unsoundness in drag-and-drop that could result in drops being rejected.
- On macOS, implemented `WindowEvent::Refresh`.
- On macOS, all `MouseCursor` variants are now implemented and the cursor will no longer reset after unfocusing.
- Removed minimum supported Rust version guarantee.

# Version 0.18.0 (2018-11-07)

- **Breaking:** `image` crate upgraded to 0.20. This is exposed as part of the `icon_loading` API.
- On Wayland, pointer events will now provide the current modifiers state.
- On Wayland, titles will now be displayed in the window header decoration.
- On Wayland, key repetition is now ended when keyboard loses focus.
- On Wayland, windows will now use more stylish and modern client side decorations.
- On Wayland, windows will use server-side decorations when available.
- **Breaking:** Added support for F16-F24 keys (variants were added to the `VirtualKeyCode` enum).
- Fixed graphical glitches when resizing on Wayland.
- On Windows, fix freezes when performing certain actions after a window resize has been triggered. Reintroduces some visual artifacts when resizing.
- Updated window manager hints under X11 to v1.5 of [Extended Window Manager Hints](https://specifications.freedesktop.org/wm-spec/wm-spec-1.5.html#idm140200472629520).
- Added `WindowBuilderExt::with_gtk_theme_variant` to X11-specific `WindowBuilder` functions.
- Fixed UTF8 handling bug in X11 `set_title` function.
- On Windows, `Window::set_cursor` now applies immediately instead of requiring specific events to occur first.
- On Windows, the `HoveredFile` and `HoveredFileCancelled` events are now implemented.
- On Windows, fix `Window::set_maximized`.
- On Windows 10, fix transparency (#260).
- On macOS, fix modifiers during key repeat.
- Implemented the `Debug` trait for `Window`, `EventsLoop`, `EventsLoopProxy` and `WindowBuilder`.
- On X11, now a `Resized` event will always be generated after a DPI change to ensure the window's logical size is consistent with the new DPI.
- Added further clarifications to the DPI docs.
- On Linux, if neither X11 nor Wayland manage to initialize, the corresponding panic now consists of a single line only.
- Add optional `serde` feature with implementations of `Serialize`/`Deserialize` for DPI types and various event types.
- Add `PartialEq`, `Eq`, and `Hash` implementations on public types that could have them but were missing them.
- On X11, drag-and-drop receiving an unsupported drop type can no longer cause the WM to freeze.
- Fix issue whereby the OpenGL context would not appear at startup on macOS Mojave (#1069).
- **Breaking:** Removed `From<NSApplicationActivationPolicy>` impl from `ActivationPolicy` on macOS.
- On macOS, the application can request the user's attention with `WindowExt::request_user_attention`.

# Version 0.17.2 (2018-08-19)

- On macOS, fix `<C-Tab>` so applications receive the event.
- On macOS, fix `<Cmd-{key}>` so applications receive the event.
- On Wayland, key press events will now be repeated.

# Version 0.17.1 (2018-08-05)

- On X11, prevent a compilation failure in release mode for versions of Rust greater than or equal to 1.30.
- Fixed deadlock that broke fullscreen mode on Windows.

# Version 0.17.0 (2018-08-02)

- Cocoa and core-graphics updates.
- Fixed thread-safety issues in several `Window` functions on Windows.
- On MacOS, the key state for modifiers key events is now properly set.
- On iOS, the view is now set correctly. This makes it possible to render things (instead of being stuck on a black screen), and touch events work again.
- Added NetBSD support.
- **Breaking:** On iOS, `UIView` is now the default root view. `WindowBuilderExt::with_root_view_class` can be used to set the root view objective-c class to `GLKView` (OpenGLES) or `MTKView` (Metal/MoltenVK).
- On iOS, the `UIApplication` is not started until `Window::new` is called.
- Fixed thread unsafety with cursor hiding on macOS.
- On iOS, fixed the size of the `JmpBuf` type used for `setjmp`/`longjmp` calls. Previously this was a buffer overflow on most architectures.
- On Windows, use cached window DPI instead of repeatedly querying the system. This fixes sporadic crashes on Windows 7.

# Version 0.16.2 (2018-07-07)

- On Windows, non-resizable windows now have the maximization button disabled. This is consistent with behavior on macOS and popular X11 WMs.
- Corrected incorrect `unreachable!` usage when guessing the DPI factor with no detected monitors.

# Version 0.16.1 (2018-07-02)

- Added logging through `log`. Logging will become more extensive over time.
- On X11 and Windows, the window's DPI factor is guessed before creating the window. This _greatly_ cuts back on unsightly auto-resizing that would occur immediately after window creation.
- Fixed X11 backend compilation for environments where `c_char` is unsigned.

# Version 0.16.0 (2018-06-25)

- Windows additionally has `WindowBuilderExt::with_no_redirection_bitmap`.
- **Breaking:** Removed `VirtualKeyCode::LMenu` and `VirtualKeyCode::RMenu`; Windows now generates `VirtualKeyCode::LAlt` and `VirtualKeyCode::RAlt` instead.
- On X11, exiting fullscreen no longer leaves the window in the monitor's top left corner.
- **Breaking:** `Window::hidpi_factor` has been renamed to `Window::get_hidpi_factor` for better consistency. `WindowEvent::HiDPIFactorChanged` has been renamed to `WindowEvent::HiDpiFactorChanged`. DPI factors are always represented as `f64` instead of `f32` now.
- The Windows backend is now DPI aware. `WindowEvent::HiDpiFactorChanged` is implemented, and `MonitorId::get_hidpi_factor` and `Window::hidpi_factor` return accurate values.
- Implemented `WindowEvent::HiDpiFactorChanged` on X11.
- On macOS, `Window::set_cursor_position` is now relative to the client area.
- On macOS, setting the maximum and minimum dimensions now applies to the client area dimensions rather than to the window dimensions.
- On iOS, `MonitorId::get_dimensions` has been implemented and both `MonitorId::get_hidpi_factor` and `Window::get_hidpi_factor` return accurate values.
- On Emscripten, `MonitorId::get_hidpi_factor` now returns the same value as `Window::get_hidpi_factor` (it previously would always return 1.0).
- **Breaking:** The entire API for sizes, positions, etc. has changed. In the majority of cases, winit produces and consumes positions and sizes as `LogicalPosition` and `LogicalSize`, respectively. The notable exception is `MonitorId` methods, which deal in `PhysicalPosition` and `PhysicalSize`. See the documentation for specifics and explanations of the types. Additionally, winit automatically conserves logical size when the DPI factor changes.
- **Breaking:** All deprecated methods have been removed. For `Window::platform_display` and `Window::platform_window`, switch to the appropriate platform-specific `WindowExt` methods. For `Window::get_inner_size_points` and `Window::get_inner_size_pixels`, use the `LogicalSize` returned by `Window::get_inner_size` and convert as needed.
- HiDPI support for Wayland.
- `EventsLoop::get_available_monitors` and `EventsLoop::get_primary_monitor` now have identical counterparts on `Window`, so this information can be acquired without an `EventsLoop` borrow.
- `AvailableMonitorsIter` now implements `Debug`.
- Fixed quirk on macOS where certain keys would generate characters at twice the normal rate when held down.
- On X11, all event loops now share the same `XConnection`.
- **Breaking:** `Window::set_cursor_state` and `CursorState` enum removed in favor of the more composable `Window::grab_cursor` and `Window::hide_cursor`. As a result, grabbing the cursor no longer automatically hides it; you must call both methods to retain the old behavior on Windows and macOS. `Cursor::NoneCursor` has been removed, as it's no longer useful.
- **Breaking:** `Window::set_cursor_position` now returns `Result<(), String>`, thus allowing for `Box<Error>` conversion via `?`.

# Version 0.15.1 (2018-06-13)

- On X11, the `Moved` event is no longer sent when the window is resized without changing position.
- `MouseCursor` and `CursorState` now implement `Default`.
- `WindowBuilder::with_resizable` implemented for Windows, X11, Wayland, and macOS.
- `Window::set_resizable` implemented for Windows, X11, Wayland, and macOS.
- On X11, if the monitor's width or height in millimeters is reported as 0, the DPI is now 1.0 instead of +inf.
- On X11, the environment variable `WINIT_HIDPI_FACTOR` has been added for overriding DPI factor.
- On X11, enabling transparency no longer causes the window contents to flicker when resizing.
- On X11, `with_override_redirect` now actually enables override redirect.
- macOS now generates `VirtualKeyCode::LAlt` and `VirtualKeyCode::RAlt` instead of `None` for both.
- On macOS, `VirtualKeyCode::RWin` and `VirtualKeyCode::LWin` are no longer switched.
- On macOS, windows without decorations can once again be resized.
- Fixed race conditions when creating an `EventsLoop` on X11, most commonly manifesting as "[xcb] Unknown sequence number while processing queue".
- On macOS, `CursorMoved` and `MouseInput` events are only generated if they occurs within the window's client area.
- On macOS, resizing the window no longer generates a spurious `MouseInput` event.

# Version 0.15.0 (2018-05-22)

- `Icon::to_cardinals` is no longer public, since it was never supposed to be.
- Wayland: improve diagnostics if initialization fails
- Fix some system event key doesn't work when focused, do not block keyevent forward to system on macOS
- On X11, the scroll wheel position is now correctly reset on i3 and other WMs that have the same quirk.
- On X11, `Window::get_current_monitor` now reliably returns the correct monitor.
- On X11, `Window::hidpi_factor` returns values from XRandR rather than the inaccurate values previously queried from the core protocol.
- On X11, the primary monitor is detected correctly even when using versions of XRandR less than 1.5.
- `MonitorId` now implements `Debug`.
- Fixed bug on macOS where using `with_decorations(false)` would cause `set_decorations(true)` to produce a transparent titlebar with no title.
- Implemented `MonitorId::get_position` on macOS.
- On macOS, `Window::get_current_monitor` now returns accurate values.
- Added `WindowBuilderExt::with_resize_increments` to macOS.
- **Breaking:** On X11, `WindowBuilderExt::with_resize_increments` and `WindowBuilderExt::with_base_size` now take `u32` values rather than `i32`.
- macOS keyboard handling has been overhauled, allowing for the use of dead keys, IME, etc. Right modifier keys are also no longer reported as being left.
- Added the `Window::set_ime_spot(x: i32, y: i32)` method, which is implemented on X11 and macOS.
- **Breaking**: `os::unix::WindowExt::send_xim_spot(x: i16, y: i16)` no longer exists. Switch to the new `Window::set_ime_spot(x: i32, y: i32)`, which has equivalent functionality.
- Fixed detection of `Pause` and `Scroll` keys on Windows.
- On Windows, alt-tabbing while the cursor is grabbed no longer makes it impossible to re-grab the cursor.
- On Windows, using `CursorState::Hide` when the cursor is grabbed now ungrabs the cursor first.
- Implemented `MouseCursor::NoneCursor` on Windows.
- Added `WindowBuilder::with_always_on_top` and `Window::set_always_on_top`. Implemented on Windows, macOS, and X11.
- On X11, `WindowBuilderExt` now has `with_class`, `with_override_redirect`, and `with_x11_window_type` to allow for more control over window creation. `WindowExt` additionally has `set_urgent`.
- More hints are set by default on X11, including `_NET_WM_PID` and `WM_CLIENT_MACHINE`. Note that prior to this, the `WM_CLASS` hint was automatically set to whatever value was passed to `with_title`. It's now set to the executable name to better conform to expectations and the specification; if this is undesirable, you must explicitly use `WindowBuilderExt::with_class`.

# Version 0.14.0 (2018-05-09)

- Created the `Copy`, `Paste` and `Cut` `VirtualKeyCode`s and added support for them on X11 and Wayland
- Fix `.with_decorations(false)` in macOS
- On Mac, `NSWindow` and supporting objects might be alive long after they were `closed` which resulted in apps consuming more heap then needed. Mainly it was affecting multi window applications. Not expecting any user visible change of behaviour after the fix.
- Fix regression of Window platform extensions for macOS where `NSFullSizeContentViewWindowMask` was not being correctly applied to `.fullsize_content_view`.
- Corrected `get_position` on Windows to be relative to the screen rather than to the taskbar.
- Corrected `Moved` event on Windows to use position values equivalent to those returned by `get_position`. It previously supplied client area positions instead of window positions, and would additionally interpret negative values as being very large (around `u16::MAX`).
- Implemented `Moved` event on macOS.
- On X11, the `Moved` event correctly use window positions rather than client area positions. Additionally, a stray `Moved` that unconditionally accompanied `Resized` with the client area position relative to the parent has been eliminated; `Moved` is still received alongside `Resized`, but now only once and always correctly.
- On Windows, implemented all variants of `DeviceEvent` other than `Text`. Mouse `DeviceEvent`s are now received even if the window isn't in the foreground.
- `DeviceId` on Windows is no longer a unit struct, and now contains a `u32`. For `WindowEvent`s, this will always be 0, but on `DeviceEvent`s it will be the handle to that device. `DeviceIdExt::get_persistent_identifier` can be used to acquire a unique identifier for that device that persists across replugs/reboots/etc.
- Corrected `run_forever` on X11 to stop discarding `Awakened` events.
- Various safety and correctness improvements to the X11 backend internals.
- Fixed memory leak on X11 every time the mouse entered the window.
- On X11, drag and drop now works reliably in release mode.
- Added `WindowBuilderExt::with_resize_increments` and `WindowBuilderExt::with_base_size` to X11, allowing for more optional hints to be set.
- Rework of the wayland backend, migrating it to use [Smithay's Client Toolkit](https://github.com/Smithay/client-toolkit).
- Added `WindowBuilder::with_window_icon` and `Window::set_window_icon`, finally making it possible to set the window icon on Windows and X11. The `icon_loading` feature can be enabled to allow for icons to be easily loaded; see example program `window_icon.rs` for usage.
- Windows additionally has `WindowBuilderExt::with_taskbar_icon` and `WindowExt::set_taskbar_icon`.
- On Windows, fix panic when trying to call `set_fullscreen(None)` on a window that has not been fullscreened prior.

# Version 0.13.1 (2018-04-26)

- Ensure necessary `x11-dl` version is used.

# Version 0.13.0 (2018-04-25)

- Implement `WindowBuilder::with_maximized`, `Window::set_fullscreen`, `Window::set_maximized` and `Window::set_decorations` for MacOS.
- Implement `WindowBuilder::with_maximized`, `Window::set_fullscreen`, `Window::set_maximized` and `Window::set_decorations` for Windows.
- On Windows, `WindowBuilder::with_fullscreen` no longer changing monitor display resolution.
- Overhauled X11 window geometry calculations. `get_position` and `set_position` are more universally accurate across different window managers, and `get_outer_size` actually works now.
- Fixed SIGSEGV/SIGILL crashes on macOS caused by stabilization of the `!` (never) type.
- Implement `WindowEvent::HiDPIFactorChanged` for macOS
- On X11, input methods now work completely out of the box, no longer requiring application developers to manually call `setlocale`. Additionally, when input methods are started, stopped, or restarted on the server end, it's correctly handled.
- Implemented `Refresh` event on Windows.
- Properly calculate the minimum and maximum window size on Windows, including window decorations.
- Map more `MouseCursor` variants to cursor icons on Windows.
- Corrected `get_position` on macOS to return outer frame position, not content area position.
- Corrected `set_position` on macOS to set outer frame position, not content area position.
- Added `get_inner_position` method to `Window`, which gets the position of the window's client area. This is implemented on all applicable platforms (all desktop platforms other than Wayland, where this isn't possible).
- **Breaking:** the `Closed` event has been replaced by `CloseRequested` and `Destroyed`. To migrate, you typically just need to replace all usages of `Closed` with `CloseRequested`; see example programs for more info. The exception is iOS, where `Closed` must be replaced by `Destroyed`.

# Version 0.12.0 (2018-04-06)

- Added subclass to macos windows so they can be made resizable even with no decorations.
- Dead keys now work properly on X11, no longer resulting in a panic.
- On X11, input method creation first tries to use the value from the user's `XMODIFIERS` environment variable, so application developers should no longer need to manually call `XSetLocaleModifiers`. If that fails, fallbacks are tried, which should prevent input method initialization from ever outright failing.
- Fixed thread safety issues with input methods on X11.
- Add support for `Touch` for win32 backend.
- Fixed `Window::get_inner_size` and friends to return the size in pixels instead of points when using HIDPI displays on OSX.

# Version 0.11.3 (2018-03-28)

- Added `set_min_dimensions` and `set_max_dimensions` methods to `Window`, and implemented on Windows, X11, Wayland, and OSX.
- On X11, dropping a `Window` actually closes it now, and clicking the window's × button (or otherwise having the WM signal to close it) will result in the window closing.
- Added `WindowBuilderExt` methods for macos: `with_titlebar_transparent`,
  `with_title_hidden`, `with_titlebar_buttons_hidden`,
  `with_fullsize_content_view`.
- Mapped X11 numpad keycodes (arrows, Home, End, PageUp, PageDown, Insert and Delete) to corresponding virtual keycodes

# Version 0.11.2 (2018-03-06)

- Impl `Hash`, `PartialEq`, and `Eq` for `events::ModifiersState`.
- Implement `MonitorId::get_hidpi_factor` for MacOS.
- Added method `os::macos::MonitorIdExt::get_nsscreen() -> *mut c_void` that gets a `NSScreen` object matching the monitor ID.
- Send `Awakened` event on Android when event loop is woken up.

# Version 0.11.1 (2018-02-19)

- Fixed windows not receiving mouse events when click-dragging the mouse outside the client area of a window, on Windows platforms.
- Added method `os::android::EventsLoopExt:set_suspend_callback(Option<Box<Fn(bool) -> ()>>)` that allows glutin to register a callback when a suspend event happens

# Version 0.11.0 (2018-02-09)

- Implement `MonitorId::get_dimensions` for Android.
- Added method `os::macos::WindowBuilderExt::with_movable_by_window_background(bool)` that allows to move a window without a titlebar - `with_decorations(false)`
- Implement `Window::set_fullscreen`, `Window::set_maximized` and `Window::set_decorations` for Wayland.
- Added `Caret` as VirtualKeyCode and support OSX ^-Key with german input.

# Version 0.10.1 (2018-02-05)

_Yanked_

# Version 0.10.0 (2017-12-27)

- Add support for `Touch` for emscripten backend.
- Added support for `DroppedFile`, `HoveredFile`, and `HoveredFileCancelled` to X11 backend.
- **Breaking:** `unix::WindowExt` no longer returns pointers for things that aren't actually pointers; `get_xlib_window` now returns `Option<std::os::raw::c_ulong>` and `get_xlib_screen_id` returns `Option<std::os::raw::c_int>`. Additionally, methods that previously returned `libc::c_void` have been changed to return `std::os::raw::c_void`, which are not interchangeable types, so users wanting the former will need to explicitly cast.
- Added `set_decorations` method to `Window` to allow decorations to be toggled after the window is built. Presently only implemented on X11.
- Raised the minimum supported version of Rust to 1.20 on MacOS due to usage of associated constants in new versions of cocoa and core-graphics.
- Added `modifiers` field to `MouseInput`, `MouseWheel`, and `CursorMoved` events to track the modifiers state (`ModifiersState`).
- Fixed the emscripten backend to return the size of the canvas instead of the size of the window.

# Version 0.9.0 (2017-12-01)

- Added event `WindowEvent::HiDPIFactorChanged`.
- Added method `MonitorId::get_hidpi_factor`.
- Deprecated `get_inner_size_pixels` and `get_inner_size_points` methods of `Window` in favor of
`get_inner_size`.
- **Breaking:** `EventsLoop` is `!Send` and `!Sync` because of platform-dependant constraints,
  but `Window`, `WindowId`, `DeviceId` and `MonitorId` guaranteed to be `Send`.
- `MonitorId::get_position` now returns `(i32, i32)` instead of `(u32, u32)`.
- Rewrite of the wayland backend to use wayland-client-0.11
- Support for dead keys on wayland for keyboard utf8 input
- Monitor enumeration on Windows is now implemented using `EnumDisplayMonitors` instead of
`EnumDisplayDevices`. This changes the value returned by `MonitorId::get_name()`.
- On Windows added `MonitorIdExt::hmonitor` method
- Impl `Clone` for `EventsLoopProxy`
- `EventsLoop::get_primary_monitor()` on X11 will fallback to any available monitor if no primary is found
- Support for touch event on wayland
- `WindowEvent`s `MouseMoved`, `MouseEntered`, and `MouseLeft` have been renamed to
`CursorMoved`, `CursorEntered`, and `CursorLeft`.
- New `DeviceEvent`s added, `MouseMotion` and `MouseWheel`.
- Send `CursorMoved` event after `CursorEntered` and `Focused` events.
- Add support for `ModifiersState`, `MouseMove`, `MouseInput`, `MouseMotion` for emscripten backend.

# Version 0.8.3 (2017-10-11)

- Fixed issue of calls to `set_inner_size` blocking on Windows.
- Mapped `ISO_Left_Tab` to `VirtualKeyCode::Tab` to make the key work with modifiers
- Fixed the X11 backed on 32bit targets

# Version 0.8.2 (2017-09-28)

- Uniformize keyboard scancode values accross Wayland and X11 (#297).
- Internal rework of the wayland event loop
- Added method `os::linux::WindowExt::is_ready`

# Version 0.8.1 (2017-09-22)

- Added various methods to `os::linux::EventsLoopExt`, plus some hidden items necessary to make
  glutin work.

# Version 0.8.0 (2017-09-21)

- Added `Window::set_maximized`, `WindowAttributes::maximized` and `WindowBuilder::with_maximized`.
- Added `Window::set_fullscreen`.
- Changed `with_fullscreen` to take a `Option<MonitorId>` instead of a `MonitorId`.
- Removed `MonitorId::get_native_identifer()` in favor of platform-specific traits in the `os`
  module.
- Changed `get_available_monitors()` and `get_primary_monitor()` to be methods of `EventsLoop`
  instead of stand-alone methods.
- Changed `EventsLoop` to be tied to a specific X11 or Wayland connection.
- Added a `os::linux::EventsLoopExt` trait that makes it possible to configure the connection.
- Fixed the emscripten code, which now compiles.
- Changed the X11 fullscreen code to use `xrandr` instead of `xxf86vm`.
- Fixed the Wayland backend to produce `Refresh` event after window creation.
- Changed the `Suspended` event to be outside of `WindowEvent`.
- Fixed the X11 backend sometimes reporting the wrong virtual key (#273).
