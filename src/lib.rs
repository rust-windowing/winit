//! Winit is a cross-platform window creation and event loop management library.
//!
//! # Building windows
//!
//! Before you can create a [`Window`], you first need to build an [`EventLoop`]. This is done with
//! the [`EventLoop::new()`] function.
//!
//! ```no_run
//! use winit::event_loop::EventLoop;
//!
//! # // Intentionally use `fn main` for clarity
//! fn main() {
//!     let event_loop = EventLoop::new().unwrap();
//!     // ...
//! }
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
//! [`DeviceEvent`]. You can also create and handle your own custom [`Event::UserEvent`]s, if
//! desired.
//!
//! You can retrieve events by calling [`EventLoop::run_app()`]. This function will
//! dispatch events for every [`Window`] that was created with that particular [`EventLoop`], and
//! will run until [`exit()`] is used, at which point [`Event::LoopExiting`].
//!
//! Winit no longer uses a `EventLoop::poll_events() -> impl Iterator<Event>`-based event loop
//! model, since that can't be implemented properly on some platforms (e.g web, iOS) and works
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
//! use winit::window::{Window, WindowId};
//!
//! #[derive(Default)]
//! struct App {
//!     window: Option<Window>,
//! }
//!
//! impl ApplicationHandler for App {
//!     fn resumed(&mut self, event_loop: &ActiveEventLoop) {
//!         self.window = Some(event_loop.create_window(Window::default_attributes()).unwrap());
//!     }
//!
//!     fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
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
//! # // Intentionally use `fn main` for clarity
//! fn main() {
//!     let event_loop = EventLoop::new().unwrap();
//!
//!     // ControlFlow::Poll continuously runs the event loop, even if the OS hasn't
//!     // dispatched any events. This is ideal for games and similar applications.
//!     event_loop.set_control_flow(ControlFlow::Poll);
//!
//!     // ControlFlow::Wait pauses the event loop if no events are available to process.
//!     // This is ideal for non-game applications that only update in response to user
//!     // input, and uses significantly less power/CPU time than ControlFlow::Poll.
//!     event_loop.set_control_flow(ControlFlow::Wait);
//!
//!     let mut app = App::default();
//!     event_loop.run_app(&mut app);
//! }
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
//! * `rwh_04`: Implement `raw-window-handle v0.4` traits.
//! * `rwh_05`: Implement `raw-window-handle v0.5` traits.
//! * `rwh_06`: Implement `raw-window-handle v0.6` traits.
//! * `serde`: Enables serialization/deserialization of certain types with [Serde](https://crates.io/crates/serde).
//! * `mint`: Enables mint (math interoperability standard types) conversions.
//!
//! See the [`platform`] module for documentation on platform-specific cargo
//! features.
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
//! [`Event::LoopExiting`]: event::Event::LoopExiting
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

#[cfg(feature = "rwh_04")]
pub use rwh_04 as raw_window_handle_04;
#[cfg(feature = "rwh_05")]
pub use rwh_05 as raw_window_handle_05;
#[cfg(feature = "rwh_06")]
pub use rwh_06 as raw_window_handle;

// Re-export DPI types so that users don't have to put it in Cargo.toml.
#[doc(inline)]
pub use dpi;

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
