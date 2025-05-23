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

- Add `ActiveEventLoop::create_proxy()`.
- On Web, add `ActiveEventLoopExtWeb::is_cursor_lock_raw()` to determine if
  `DeviceEvent::MouseMotion` is returning raw data, not OS accelerated, when using
  `CursorGrabMode::Locked`.
- On Web, implement `MonitorHandle` and `VideoModeHandle`.

  Without prompting the user for permission, only the current monitor is returned. But when
  prompting and being granted permission through
  `ActiveEventLoop::request_detailed_monitor_permission()`, access to all monitors and their
  details is available. Handles created with "detailed monitor permissions" can be used in
  `Window::set_fullscreen()` as well.

  Keep in mind that handles do not auto-upgrade after permissions are granted and have to be
  re-created to make full use of this feature.
- Implement `Clone`, `Copy`, `Debug`, `Deserialize`, `Eq`, `Hash`, `Ord`, `PartialEq`, `PartialOrd`
  and `Serialize` on many types.
- Add `MonitorHandle::current_video_mode()`.
- Add `ApplicationHandlerExtMacOS` trait, and a `macos_handler` method to `ApplicationHandler` which returns a `dyn ApplicationHandlerExtMacOS` which allows for macOS specific extensions to winit.
- Add a `standard_key_binding` method to the `ApplicationHandlerExtMacOS` trait. This allows handling of standard keybindings such as "go to end of line" on macOS.
- On macOS, add `WindowExtMacOS::set_unified_titlebar` and `WindowAttributesMacOS::with_unified_titlebar`
  to use a larger style of titlebar.
- Add `WindowId::into_raw()` and `from_raw()`.
- Add `PointerKind`, `PointerSource`, `ButtonSource`, `FingerId`, `primary` and `position` to all
  pointer events as part of the pointer event overhaul.
- Add `DeviceId::into_raw()` and `from_raw()`.
- Added `Window::surface_position`, which is the position of the surface inside the window.
- Added `Window::safe_area`, which describes the area of the surface that is unobstructed.
- On X11, Wayland, Windows and macOS, improved scancode conversions for more obscure key codes.
- Add ability to make non-activating window on macOS using `NSPanel` with `NSWindowStyleMask::NonactivatingPanel`.
- Implement `MonitorHandleProvider` for `MonitorHandle` to access common monitor API.
- On X11, set an "area" attribute on XIM input connection to convey the cursor area.
- Implement `CustomCursorProvider` for `CustomCursor` to access cursor API.
- Add `CustomCursorSource::Url`, `CustomCursorSource::from_animation`.
- Implement `CustomIconProvider` for `RgbaIcon`.
- Add `icon` module that exposes winit's icon API.
- `VideoMode::new` to create a `VideoMode`.
- `keyboard::ModifiersKey` to track which modifier is exactly pressed.
- `ActivationToken::as_raw` to get a ref to raw token.
- Each platform now has corresponding `WindowAttributes` struct instead of trait extension.
- On Windows, update side-aware `event::Modifiers` information on state change.
- On Windows, added <kbd>AltGr</kbd> as a separate modifier (though currently <kbd>AltGr</kbd>+<kbd>LCtrl</kbd> can't be differentiated from just <kbd>AltGr</kbd>).

### Changed

- Change `ActiveEventLoop` and `Window` to be traits, and added `cast_ref`/`cast_mut`/`cast`
  methods to extract the backend type from those.
- `ActiveEventLoop::create_window` now returns `Box<dyn Window>`.
- `ApplicationHandler` now uses `dyn ActiveEventLoop`.
- On Web, let events wake up event loop immediately when using `ControlFlow::Poll`.
- Bump MSRV from `1.70` to `1.80`.
- Changed `ApplicationHandler::user_event` to `user_wake_up`, removing the
  generic user event.

  Winit will now only indicate that wake up happened, you will have to pair
  this with an external mechanism like `std::sync::mpsc::channel` if you want
  to send specific data to be processed on the main thread.
- Changed `EventLoopProxy::send_event` to `EventLoopProxy::wake_up`, it now
  only wakes up the loop.
- On X11, implement smooth resizing through the sync extension API.
- `ApplicationHandler::can_create|destroy_surfaces()` was split off from
  `ApplicationHandler::resumed/suspended()`.

  `ApplicationHandler::can_create_surfaces()` should, for portability reasons
  to Android, be the only place to create render surfaces.

  `ApplicationHandler::resumed/suspended()` are now only emitted by iOS, Web
  and Android, and now signify actually resuming/suspending the application.
- Rename `platform::web::*ExtWebSys` to `*ExtWeb`.
- Change signature of `EventLoop::run_app`, `EventLoopExtPumpEvents::pump_app_events` and
  `EventLoopExtRunOnDemand::run_app_on_demand` to accept a `impl ApplicationHandler` directly,
  instead of requiring a `&mut` reference to it.
- On Web, `Window::canvas()` now returns a reference.
- On Web, `CursorGrabMode::Locked` now lets `DeviceEvent::MouseMotion` return raw data, not OS
  accelerated, if the browser supports it.
- `(Active)EventLoop::create_custom_cursor()` now returns a `Result<CustomCursor, ExternalError>`.
- Changed how `ModifiersState` is serialized by Serde.
- `VideoModeHandle::refresh_rate_millihertz()` and `bit_depth()` now return a `Option<NonZero*>`.
- `MonitorHandle::position()` now returns an `Option`.
- On macOS, remove custom application delegates. You are now allowed to override the
  application delegate yourself.
- On X11, remove our dependency on libXcursor. (#3749)
- Renamed the following APIs to make it clearer that the sizes apply to the underlying surface:
  - `WindowEvent::Resized` to `SurfaceResized`.
  - `InnerSizeWriter` to `SurfaceSizeWriter`.
  - `WindowAttributes.inner_size` to `surface_size`.
  - `WindowAttributes.min_inner_size` to `min_surface_size`.
  - `WindowAttributes.max_inner_size` to `max_surface_size`.
  - `WindowAttributes.resize_increments` to `surface_resize_increments`.
  - `WindowAttributes::with_inner_size` to `with_surface_size`.
  - `WindowAttributes::with_min_inner_size` to `with_min_surface_size`.
  - `WindowAttributes::with_max_inner_size` to `with_max_surface_size`.
  - `WindowAttributes::with_resize_increments` to `with_surface_resize_increments`.
  - `Window::inner_size` to `surface_size`.
  - `Window::request_inner_size` to `request_surface_size`.
  - `Window::set_min_inner_size` to `set_min_surface_size`.
  - `Window::set_max_inner_size` to `set_max_surface_size`.

  To migrate, you can probably just replace all instances of `inner_size` with `surface_size` in your codebase.
- Every event carrying a `DeviceId` now uses `Option<DeviceId>` instead. A `None` value signifies that the
  device can't be uniquely identified.
- Pointer `WindowEvent`s were overhauled. The new events can handle any type of pointer, serving as
  a single pointer input source. Now your application can handle any pointer type without having to
  explicitly handle e.g. `Touch`:
  - Rename `CursorMoved` to `PointerMoved`.
  - Rename `CursorEntered` to `PointerEntered`.
  - Rename `CursorLeft` to `PointerLeft`.
  - Rename `MouseInput` to `PointerButton`.
  - Add `primary` to every `PointerEvent` as a way to identify discard non-primary pointers in a
    multi-touch interaction.
  - Add `position` to every `PointerEvent`.
  - `PointerMoved` is **not sent** after `PointerEntered` anymore.
  - Remove `Touch`, which is folded into the `Pointer*` events.
  - New `PointerKind` added to `PointerEntered` and `PointerLeft`, signifying which pointer type is
    the source of this event.
  - New `PointerSource` added to `PointerMoved`, similar to `PointerKind` but holding additional
    data.
  - New `ButtonSource` added to `PointerButton`, similar to `PointerKind` but holding pointer type
    specific buttons. Use `ButtonSource::mouse_button()` to easily normalize any pointer button
    type to a generic mouse button.
  - New `FingerId` added to `PointerKind::Touch` and `PointerSource::Touch` able to uniquely
    identify a finger in a multi-touch interaction. Replaces the old `Touch::id`.
  - In the same spirit rename `DeviceEvent::MouseMotion` to `PointerMotion`.
  - Remove `Force::Calibrated::altitude_angle`.
- On X11, use bottom-right corner for IME hotspot in `Window::set_ime_cursor_area`.
- On macOS and iOS, no longer emit `ScaleFactorChanged` upon window creation.
- On macOS, no longer emit `Focused` upon window creation.
- On iOS, emit more events immediately, instead of queuing them.
- Update `smol_str` to version `0.3`
- Rename `VideoModeHandle` to `VideoMode`, now it only stores plain data.
- Make `Fullscreen::Exclusive` contain `(MonitorHandle, VideoMode)`.
- Reworked the file drag-and-drop API.

  The `WindowEvent::DroppedFile`, `WindowEvent::HoveredFile` and `WindowEvent::HoveredFileCancelled`
  events have been removed, and replaced with `WindowEvent::DragEntered`, `WindowEvent::DragMoved`,
  `WindowEvent::DragDropped` and `WindowEvent::DragLeft`.

  The old drag-and-drop events were emitted once per file. This occurred when files were *first*
  hovered over the window, dropped, or left the window. The new drag-and-drop events are emitted
  once per set of files dragged, and include a list of all dragged files. They also include the
  pointer position.

  The rough correspondence is:
  - `WindowEvent::HoveredFile` -> `WindowEvent::DragEntered`
  - `WindowEvent::DroppedFile` -> `WindowEvent::DragDropped`
  - `WindowEvent::HoveredFileCancelled` -> `WindowEvent::DragLeft`

  The `WindowEvent::DragMoved` event is entirely new, and is emitted whenever the pointer moves
  whilst files are being dragged over the window. It doesn't contain any file paths, just the
  pointer position.
- Updated `objc2` to `v0.6`.
- Updated `windows-sys` to `v0.59`.
  - To match the corresponding changes in `windows-sys`, the `HWND`, `HMONITOR`, and `HMENU` types
    now alias to `*mut c_void` instead of `isize`.
- Removed `KeyEventExtModifierSupplement`, and made the fields `text_with_all_modifiers` and
  `key_without_modifiers` public on `KeyEvent` instead.
- Move `window::Fullscreen` to `monitor::Fullscreen`.
- Renamed "super" key to "meta", to match the naming in the W3C specification.
  `NamedKey::Super` still exists, but it's non-functional and deprecated, `NamedKey::Meta` should be used instead.
- Move `IconExtWindows` into `WinIcon`.
- Move `EventLoopExtPumpEvents` and `PumpStatus` from platform module to `winit::event_loop::pump_events`.
- Move `EventLoopExtRunOnDemand` from platform module to `winit::event_loop::run_on_demand`.

### Removed

- Remove `Event`.
- Remove already deprecated APIs:
  - `EventLoop::create_window()`
  - `EventLoop::run`.
  - `EventLoopBuilder::new()`
  - `EventLoopExtPumpEvents::pump_events`.
  - `EventLoopExtRunOnDemand::run_on_demand`.
  - `VideoMode`
  - `WindowAttributes::new()`
  - `Window::set_cursor_icon()`
- On iOS, remove `platform::ios::EventLoopExtIOS` and related `platform::ios::Idiom` type.

  This feature was incomplete, and the equivalent functionality can be trivially achieved outside
  of `winit` using `objc2-ui-kit` and calling `UIDevice::currentDevice().userInterfaceIdiom()`.
- On Web, remove unused `platform::web::CustomCursorError::Animation`.
- Remove the `rwh_04` and `rwh_05` cargo feature and the corresponding `raw-window-handle` v0.4 and
  v0.5 support. v0.6 remains in place and is enabled by default.
- Remove `DeviceEvent::Added` and `DeviceEvent::Removed`.
- Remove `DeviceEvent::Motion` and `WindowEvent::AxisMotion`.
- Remove `MonitorHandle::size()` and `refresh_rate_millihertz()` in favor of
  `MonitorHandle::current_video_mode()`.
- On Android, remove all `MonitorHandle` support instead of emitting false data.
- Remove `impl From<u64> for WindowId` and `impl From<WindowId> for u64`. Replaced with
  `WindowId::into_raw()` and `from_raw()`.
- Remove `dummy()` from `WindowId` and `DeviceId`.
- Remove `WindowEvent::Touch` and `Touch` in favor of the new `PointerKind`, `PointerSource` and
 `ButtonSource` as part of the new pointer event overhaul.
- Remove `Force::altitude_angle`.
- Remove `Window::inner_position`, use the new `Window::surface_position` instead.
- Remove `CustomCursorExtWeb`, use the `CustomCursorSource`.
- Remove `CustomCursor::from_rgba`, use `CustomCursorSource` instead.
- Remove `ApplicationHandler::exited`, the event loop being shut down can now be listened to in
  the `Drop` impl on the application handler.
- Remove `NamedKey::Space`, match on `Key::Character(" ")` instead.
- Remove `PartialEq` impl for `WindowAttributes`.
- `WindowAttributesExt*` platform extensions; use `WindowAttributes*` instead.

### Fixed

- On Orbital, `MonitorHandle::name()` now returns `None` instead of a dummy name.
- On iOS, fixed `SurfaceResized` and `Window::surface_size` not reporting the size of the actual surface.
- On macOS, fixed the scancode conversion for audio volume keys.
- On macOS, fixed the scancode conversion for `IntlBackslash`.
- On macOS, fixed redundant `SurfaceResized` event at window creation.
