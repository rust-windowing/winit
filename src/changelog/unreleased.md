## Unreleased

- **Breaking:** Move `Window::new` to `ActiveEventLoop::create_window`.

    This means that you now have to create your windows inside the actively running event loop (usually the `NewEvents(StartCause::Init)` or `Resumed` events), and can no longer do it before the application has properly launched. This change is done to fix many long-standing issues on iOS and macOS.

    See the code snippet below for an example of how to migrate.

    We recognize that this is still a bit cumbersome, so to ease migration, we provide the deprecated `EventLoop::create_window`. In the future, managing the state for windows, and creating/destroying them at the right times will likely become easier to do, see [#2903](https://github.com/rust-windowing/winit/issues/2903) for some of the progress on that.
    ```rust
    // Before
    let window = Window::new(&event_loop);
    event_loop.run(|event, event_loop| {
        match event {
            // ... Handle events
        }
    });

    // After
    //
    // The window _could_ be created outside the event loop like so, but this
    // is discouraged.
    // ```
    // let window = event_loop.create_window(Window::attributes());
    // ```
    //
    // Instead, we will use an `Option` to allow the window to not be available
    // until the application is properly running.
    let mut window = None;
    event_loop.run(|event, event_loop| {
        match event {
            Event::Resumed => {
                window = Some(event_loop.create_window(Window::attributes()));
            }
            Event::Suspended => {
                window = None;
            }
            Event::WindowEvent { window_id, event } => {
                // `unwrap` is fine, the window will always be available when
                // recieving a window event.
                let window = window.as_ref().unwrap();
                // ... Handle window events
            }
            // ... Handle other events
        }
    });
    ```
- Deprecate `EventLoop::run` (and similar APIs for running the event loop) in favour of `EventLoop::run_app` (and family), and add `ApplicationHandler<T>` trait which mimics `Event<T>`.

    Winit is moving towards a trait-based API for several reasons, see [#3432](https://github.com/rust-windowing/winit/issues/3432) for details. This will have quite a large impact on how you structure your code, and while the design is not yet optimal, and will need additional work, we feel confident that this is the right way forwards, so in this release, we've tried to provide an easier update path. Please submit your feedback when migrating in [this issue](https://github.com/rust-windowing/winit/issues/TODO).

    See the code snippet below for an example of how to migrate. Note that the code does unfortunately get more verbose, but we believe that in most real-world applications this shouldn't be that big of an issue.

    ```rust
    // Before
    let mut window = None;
    let mut click_counter = 0;
    event_loop.run(|event, event_loop| {
        match event {
            Event::Resumed => {
                window = Some(event_loop.create_window(Window::attributes()));
            }
            Event::Suspended => {
                window = None;
            }
            Event::WindowEvent { window_id, event } => match event {
                WindowEvent::MouseInput { button: MouseButton::Left, state: ElementState::Pressed, .. } => {
                    click_counter += 1;
                },
                // ... Handle other window events
            },
            // ... Handle other top-level events
        }
    });

    // After
    struct App {
        window: Option<Window>,
        click_counter: i32,
    }

    impl ApplicationHandler for App {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            self.window = Some(event_loop.create_window(Window::attributes()));
        }

        fn suspended(&mut self, event_loop: &ActiveEventLoop) {
            self.window = None;
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            window_id: WindowId,
            event: WindowEvent,
        ) {
            match event {
                WindowEvent::MouseInput { button: MouseButton::Left, state: ElementState::Pressed, .. } => {
                    self.click_counter += 1;
                },
                // ... Handle other window events
            }
        }

        // ... Handle other top-level events
    }

    event_loop.run_app(App {
        window: None,
        click_counter: 0,
    });
    ```
- Move `dpi` types to its own crate, and re-export it from the root crate.
- Implement `Sync` for `EventLoopProxy<T: Send>`.
- **Breaking:** Rename `EventLoopWindowTarget` to `ActiveEventLoop`.
- **Breaking:** Remove `Deref` implementation for `EventLoop` that gave `EventLoopWindowTarget`.
- **Breaking**: Remove `WindowBuilder` in favor of `WindowAttributes`.
- **Breaking:** Removed unnecessary generic parameter `T` from `EventLoopWindowTarget`.
- On Windows, macOS, X11, Wayland and Web, implement setting images as cursors. See the `custom_cursors.rs` example.
  - **Breaking:** Remove `Window::set_cursor_icon`
  - Add `WindowBuilder::with_cursor` and `Window::set_cursor` which takes a `CursorIcon` or `CustomCursor`
  - Add `CustomCursor::from_rgba` to allow creating cursor images from RGBA data.
  - Add `CustomCursorExtWebSys::from_url` to allow loading cursor images from URLs.
  - Add `CustomCursorExtWebSys::from_animation` to allow creating animated cursors from other `CustomCursor`s.
  - Add `{Active,}EventLoop::create_custom_cursor` to load custom cursor image sources.
- On macOS, add services menu.
- **Breaking:** On Web, remove queuing fullscreen request in absence of transient activation.
- On Web, fix setting cursor icon overriding cursor visibility.
- **Breaking:** On Web, return `RawWindowHandle::WebCanvas` instead of `RawWindowHandle::Web`.
- **Breaking:** On Web, macOS and iOS, return `HandleError::Unavailable` when a window handle is not available.
- **Breaking:** Bump MSRV from `1.65` to `1.70`.
- On Web, add the ability to toggle calling `Event.preventDefault()` on `Window`.
- **Breaking:** Remove `WindowAttributes::fullscreen()` and expose as field directly.
- **Breaking:** Rename `VideoMode` to `VideoModeHandle` to represent that it doesn't hold static data.
- **Breaking:** No longer export `platform::x11::XNotSupported`.
- **Breaking:** Renamed `platform::x11::XWindowType` to `platform::x11::WindowType`.
- Add the `OwnedDisplayHandle` type for allowing safe display handle usage outside of trivial cases.
- **Breaking:** Rename `TouchpadMagnify` to `PinchGesture`, `SmartMagnify` to `DoubleTapGesture` and `TouchpadRotate` to `RotationGesture` to represent the action rather than the intent.
- on iOS, add detection support for `PinchGesture`, `DoubleTapGesture` and `RotationGesture`.
- on Windows: add `with_system_backdrop`, `with_border_color`, `with_title_background_color`, `with_title_text_color` and `with_corner_preference`
- On Windows, Remove `WS_CAPTION`, `WS_BORDER` and `WS_EX_WINDOWEDGE` styles for child windows without decorations.
- **Breaking:** Removed `EventLoopError::AlreadyRunning`, which can't happen as it is already prevented by the type system.
- Added `EventLoop::builder`, which is intended to replace the (now deprecated) `EventLoopBuilder::new`.
- **Breaking:** Changed the signature of `EventLoop::with_user_event` to return a builder.
- **Breaking:** Removed `EventLoopBuilder::with_user_event`, the functionality is now available in `EventLoop::with_user_event`.
- Add `Window::default_attributes` to get default `WindowAttributes`.
- `log` has been replaced with `tracing`. The old behavior can be emulated by setting the `log` feature on the `tracing` crate.
- On Windows, confine cursor to center of window when grabbed and hidden.
