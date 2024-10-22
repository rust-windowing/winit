//! Winit is a cross-platform window creation and event loop management library.
//!
//! # Building windows
//!
//! Before you can create a [`Window`], you first need to build an [`EventLoop`]. This is done with
//! the [`EventLoop::new()`] function.
//!
//! ```no_run
//! use winit::event_loop::EventLoop;
//! let event_loop = EventLoop::new().unwrap();
//! ```
//!
//! Then you create a [`Window`] with [`create_window`].
//!
//! # Event handling
//!
//! Once a [`Window`] has been created, it will generate different *events*. A [`Window`] object can
//! generate [`WindowEvent`]s when certain input events occur, such as a cursor moving over the
//! window or a key getting pressed while the window is focused. Devices can generate
//! [`DeviceEvent`]s, which contain unfiltered event data that isn't specific to a certain window.
//! Some user activity, like mouse movement, can generate both a [`WindowEvent`] *and* a
//! [`DeviceEvent`].
//!
//! You can retrieve events by calling [`EventLoop::run_app()`]. This function will
//! dispatch events for every [`Window`] that was created with that particular [`EventLoop`], and
//! will run until [`exit()`] is used, at which point [`exiting()`] is called.
//!
//! Winit no longer uses a `EventLoop::poll_events() -> impl Iterator<Event>`-based event loop
//! model, since that can't be implemented properly on some platforms (e.g Web, iOS) and works
//! poorly on most other platforms. However, this model can be re-implemented to an extent with
#![cfg_attr(
    any(windows_platform, macos_platform, android_platform, x11_platform, wayland_platform),
    doc = "[`EventLoopExtPumpEvents::pump_app_events()`][platform::pump_events::EventLoopExtPumpEvents::pump_app_events()]"
)]
#![cfg_attr(
    not(any(windows_platform, macos_platform, android_platform, x11_platform, wayland_platform)),
    doc = "`EventLoopExtPumpEvents::pump_app_events()`"
)]
//! [^1]. See that method's documentation for more reasons about why
//! it's discouraged beyond compatibility reasons.
//!
//!
//! ```no_run
//! use winit::application::ApplicationHandler;
//! use winit::event::WindowEvent;
//! use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
//! use winit::window::{Window, WindowId, WindowAttributes};
//!
//! #[derive(Default)]
//! struct App {
//!     window: Option<Box<dyn Window>>,
//! }
//!
//! impl ApplicationHandler for App {
//!     fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
//!         self.window = Some(event_loop.create_window(WindowAttributes::default()).unwrap());
//!     }
//!
//!     fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, id: WindowId, event: WindowEvent) {
//!         match event {
//!             WindowEvent::CloseRequested => {
//!                 println!("The close button was pressed; stopping");
//!                 event_loop.exit();
//!             },
//!             WindowEvent::RedrawRequested => {
//!                 // Redraw the application.
//!                 //
//!                 // It's preferable for applications that do not render continuously to render in
//!                 // this event rather than in AboutToWait, since rendering in here allows
//!                 // the program to gracefully handle redraws requested by the OS.
//!
//!                 // Draw.
//!
//!                 // Queue a RedrawRequested event.
//!                 //
//!                 // You only need to call this if you've determined that you need to redraw in
//!                 // applications which do not always need to. Applications that redraw continuously
//!                 // can render here instead.
//!                 self.window.as_ref().unwrap().request_redraw();
//!             }
//!             _ => (),
//!         }
//!     }
//! }
//!
//! let event_loop = EventLoop::new().unwrap();
//!
//! // ControlFlow::Poll continuously runs the event loop, even if the OS hasn't
//! // dispatched any events. This is ideal for games and similar applications.
//! event_loop.set_control_flow(ControlFlow::Poll);
//!
//! // ControlFlow::Wait pauses the event loop if no events are available to process.
//! // This is ideal for non-game applications that only update in response to user
//! // input, and uses significantly less power/CPU time than ControlFlow::Poll.
//! event_loop.set_control_flow(ControlFlow::Wait);
//!
//! let mut app = App::default();
//! event_loop.run_app(&mut app);
//! ```
//!
//! [`WindowEvent`] has a [`WindowId`] member. In multi-window environments, it should be
//! compared to the value returned by [`Window::id()`] to determine which [`Window`]
//! dispatched the event.
//!
//! # Drawing on the window
//!
//! Winit doesn't directly provide any methods for drawing on a [`Window`]. However, it allows you
//! to retrieve the raw handle of the window and display (see the [`platform`] module and/or the
//! [`raw_window_handle`] and [`raw_display_handle`] methods), which in turn allows
//! you to create an OpenGL/Vulkan/DirectX/Metal/etc. context that can be used to render graphics.
//!
//! Note that many platforms will display garbage data in the window's client area if the
//! application doesn't render anything to the window by the time the desktop compositor is ready to
//! display the window to the user. If you notice this happening, you should create the window with
//! [`visible` set to `false`][crate::window::WindowAttributes::with_visible] and explicitly make
//! the window visible only once you're ready to render into it.
//!
//! # UI scaling
//!
//! UI scaling is important, go read the docs for the [`dpi`] crate for an
//! introduction.
//!
//! All of Winit's functions return physical types, but can take either logical or physical
//! coordinates as input, allowing you to use the most convenient coordinate system for your
//! particular application.
//!
//! Winit will dispatch a [`ScaleFactorChanged`] event whenever a window's scale factor has changed.
//! This can happen if the user drags their window from a standard-resolution monitor to a high-DPI
//! monitor or if the user changes their DPI settings. This allows you to rescale your application's
//! UI elements and adjust how the platform changes the window's size to reflect the new scale
//! factor. If a window hasn't received a [`ScaleFactorChanged`] event, its scale factor
//! can be found by calling [`window.scale_factor()`].
//!
//! [`ScaleFactorChanged`]: event::WindowEvent::ScaleFactorChanged
//! [`window.scale_factor()`]: window::Window::scale_factor
//!
//! # Cargo Features
//!
//! Winit provides the following Cargo features:
//!
//! * `x11` (enabled by default): On Unix platforms, enables the X11 backend.
//! * `wayland` (enabled by default): On Unix platforms, enables the Wayland backend.
//! * `rwh_06`: Implement `raw-window-handle v0.6` traits.
//! * `serde`: Enables serialization/deserialization of certain types with [Serde](https://crates.io/crates/serde).
//! * `mint`: Enables mint (math interoperability standard types) conversions.
//!
//! See the [`platform`] module for documentation on platform-specific cargo
//! features.
//!
//! # Platform/Architecture Support
//!
//! Platform support on `winit` has two tiers: Tier 1 and Tier 2.
//!
//! - Tier 1 is **guaranteed to work**. Targets in this tier are actively tested both in CI and by
//!   maintainers.
//! - Tier 2 is **guaranteed to build**. Code compilation is tested in CI, but deeper testing is not
//!   done.
//!
//! Please open an issue if you would like to add a Tier 2 target, or if you would
//! like a Tier 2 target moved to Tier 1.
//!
//! ## Tier 1 Targets
//!
//! |Target Name                    |Target Triple                       |APIs           |
//! |-------------------------------|------------------------------------|---------------|
//! |32-Bit x86 Windows with MSVC   |`i686-pc-windows-msvc`              |Win32          |
//! |64-Bit x86 Windows with MSVC   |`x86_64-pc-windows-msvc`            |Win32          |
//! |32-Bit x86 Windows with glibc  |`i686-pc-windows-gnu`               |Win32          |
//! |64-Bit x86 Windows with glibc  |`x86_64-pc-windows-gnu`             |Win32          |
//! |32-Bit x86 Linux with glibc    |`i686-unknown-linux-gnu`            |X11, Wayland   |
//! |64-Bit x86 Linux with glibc    |`x86_64-unknown-linux-gnu`          |X11, Wayland   |
//! |64-Bit ARM Android             |`aarch64-linux-android`             |Android        |
//! |64-Bit x86 Redox OS            |`x86_64-unknown-redox`              |Orbital        |
//! |32-Bit x86 Redox OS            |`i686-unknown-redox`                |Orbital        |
//! |64-Bit ARM Redox OS            |`aarch64-unknown-redox`             |Orbital        |
//! |64-bit x64 macOS               |`x86_64-apple-darwin`               |AppKit         |
//! |64-bit ARM macOS               |`aarch64-apple-darwin`              |AppKit         |
//! |32-bit Wasm Web browser        |`wasm32-unknown-unknown`            |`wasm-bindgen` |
//!
//! ## Tier 2 Targets
//!
//! |Target Name                         |Target Triple                       |APIs           |
//! |------------------------------------|------------------------------------|---------------|
//! |64-Bit ARM Windows with MSVC        |`aarch64-pc-windows-msvc`           |Win32          |
//! |32-Bit x86 Windows 7 with MSVC      |`i686-win7-windows-msvc`            |Win32          |
//! |64-Bit x86 Windows 7 with MSVC      |`x86_64-win7-windows-msvc`          |Win32          |
//! |64-bit x86 Linux with Musl          |`x86_64-unknown-linux-musl`         |X11, Wayland   |
//! |64-bit x86 Linux with 32-bit glibc  |`x86_64-unknown-linux-gnux32`       |X11, Wayland   |
//! |64-bit x86 Android                  |`x86_64-linux-android`              |Android        |
//! |64-bit x64 iOS                      |`x86_64-apple-ios`                  |UIKit          |
//! |64-bit ARM iOS                      |`aarch64-apple-ios`                 |UIKit          |
//! |64-bit ARM Mac Catalyst             |`aarch64-apple-ios-macabi`          |UIKit          |
//! |32-bit x86 Android                  |`i686-linux-android`                |Android        |
//! |64-bit x86 FreeBSD                  |`x86_64-unknown-freebsd`            |X11, Wayland   |
//! |64-bit x86 NetBSD                   |`x86_64-unknown-netbsd`             |X11            |
//! |32-bit x86 Linux with Musl          |`i686-unknown-linux-musl`           |X11, Wayland   |
//! |64-bit RISC-V Linux with glibc      |`riscv64gc-unknown-linux-gnu`       |X11, Wayland   |
//! |64-bit ARM Linux with glibc         |`aarch64-unknown-linux-gnu`         |X11, Wayland   |
//! |64-bit ARM Linux with Musl          |`aarch64-unknown-linux-musl`        |X11, Wayland   |
//! |64-bit PowerPC Linux with glibc     |`powerpc64le-unknown-linux-gnu`     |X11, Wayland   |
//! |32-Bit ARM Linux with glibc         |`armv5te-unknown-linux-gnueabi`     |X11, Wayland   |
//! |64-Bit Linux on IBM Supercomputers  |`s390x-unknown-linux-gnu`           |X11, Wayland   |
//! |32-bit ARM Android                  |`arm-linux-androideabi`             |Android        |
//! |64-bit SPARC Linux with glibc       |`sparc64-unknown-linux-gnu`         |X11, Wayland   |
//!
//! [`EventLoop`]: event_loop::EventLoop
//! [`EventLoop::new()`]: event_loop::EventLoop::new
//! [`EventLoop::run_app()`]: event_loop::EventLoop::run_app
//! [`exit()`]: event_loop::ActiveEventLoop::exit
//! [`Window`]: window::Window
//! [`WindowId`]: window::WindowId
//! [`WindowAttributes`]: window::WindowAttributes
//! [window_new]: window::Window::new
//! [`create_window`]: event_loop::ActiveEventLoop::create_window
//! [`Window::id()`]: window::Window::id
//! [`WindowEvent`]: event::WindowEvent
//! [`DeviceEvent`]: event::DeviceEvent
//! [`Event::UserEvent`]: event::Event::UserEvent
//! [`exiting()`]: crate::application::ApplicationHandler::exiting
//! [`raw_window_handle`]: ./window/struct.Window.html#method.raw_window_handle
//! [`raw_display_handle`]: ./window/struct.Window.html#method.raw_display_handle
//! [^1]: `EventLoopExtPumpEvents::pump_app_events()` is only available on Windows, macOS, Android, X11 and Wayland.

#![deny(rust_2018_idioms)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(clippy::all)]
#![deny(unsafe_op_in_unsafe_fn)]
#![cfg_attr(clippy, deny(warnings))]
// Doc feature labels can be tested locally by running RUSTDOCFLAGS="--cfg=docsrs" cargo +nightly
// doc
#![cfg_attr(docsrs, feature(doc_auto_cfg, doc_cfg_hide), doc(cfg_hide(doc, docsrs)))]
#![allow(clippy::missing_safety_doc)]

// Re-export DPI types so that users don't have to put it in Cargo.toml.
#[doc(inline)]
pub use dpi;
#[cfg(feature = "rwh_06")]
pub use rwh_06 as raw_window_handle;

pub mod application;
#[cfg(any(doc, doctest, test))]
pub mod changelog;
#[macro_use]
pub mod error;
mod cursor;
pub mod event;
pub mod event_loop;
mod icon;
pub mod keyboard;
pub mod monitor;
mod platform_impl;
mod utils;
pub mod window;

pub mod platform;
