//! Winit is a cross-platform window creation and event loop management library.
//!
//! # Event handling
//!
//! Basically all of the functionality that Winit exposes requires an [`ActiveEventLoop`], which you
//! can get access to by running an [`EventLoop`] using [`EventLoop::new()`] and
//! [`EventLoop::run()`].
//!
//! Once it's running, you can create your [`Window`]s with [`ActiveEventLoop::create_window()`] by
//! passing in the desired [`WindowAttributes`].
//!
//! Once a [`Window`] has been created, it will generate different *events*. A [`Window`] object can
//! generate [`WindowEvent`]s when certain input events occur, such as a cursor moving over the
//! window or a key getting pressed while the window is focused. Devices can generate
//! [`DeviceEvent`]s, which contain unfiltered event data that isn't specific to a certain window.
//! Some user activity, like mouse movement, can generate both a [`WindowEvent`] *and* a
//! [`DeviceEvent`].
//!
//! You retrieve by implementing [`ApplicationHandler`] for a new type, which will be the state of
//! your application. The methods in this trait will continuously receive events until
//! [`ActiveEventLoop::exit()`] is used, at which point your application state will be dropped, and
//! the application shuts down.
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
//! use winit::window::{Window, WindowAttributes, WindowId};
//!
//! struct App {
//!     window: Box<dyn Window>,
//! }
//!
//! impl ApplicationHandler for App {
//!     fn window_event(
//!         &mut self,
//!         event_loop: &dyn ActiveEventLoop,
//!         id: WindowId,
//!         event: WindowEvent,
//!     ) {
//!         // Called by `EventLoop::run` when a new event happens on the window.
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
//!                 // applications which do not always need to.
//!                 self.window.request_redraw();
//!             },
//!             _ => (),
//!         }
//!     }
//! }
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a new event loop.
//!     let event_loop = EventLoop::new()?;
//!
//!     // Configure settings before launching.
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
//!     // Launch and begin running our event loop.
//!     event_loop.run(|event_loop| {
//!         // The event loop has launched, and we can initialize our UI state in this closure.
//!
//!         // Create a simple window with default attributes.
//!         let window = event_loop
//!             .create_window(WindowAttributes::default())
//!             .expect("failed creating window");
//!
//!         // Give our newly created application state to Winit, which will, when necessary, call
//!         // the `ApplicationHandler` methods configured above.
//!         App { window }
//!     })?;
//!
//!     Ok(())
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
//! * `rwh_06`: Implement `raw-window-handle v0.6` traits.
//! * `serde`: Enables serialization/deserialization of certain types with [Serde](https://crates.io/crates/serde).
//! * `mint`: Enables mint (math interoperability standard types) conversions.
//!
//! See the [`platform`] module for documentation on platform-specific cargo
//! features.
//!
//! [`ActiveEventLoop`]: event_loop::ActiveEventLoop
//! [`EventLoop`]: event_loop::EventLoop
//! [`EventLoop::new()`]: event_loop::EventLoop::new
//! [`EventLoop::run()`]: event_loop::EventLoop::run
//! [`ActiveEventLoop::exit()`]: event_loop::ActiveEventLoop::exit
//! [`Window`]: window::Window
//! [`WindowId`]: window::WindowId
//! [`WindowAttributes`]: window::WindowAttributes
//! [window_new]: window::Window::new
//! [`ActiveEventLoop::create_window()`]: event_loop::ActiveEventLoop::create_window
//! [`Window::id()`]: window::Window::id
//! [`WindowEvent`]: event::WindowEvent
//! [`DeviceEvent`]: event::DeviceEvent
//! [`ApplicationHandler`]: application::ApplicationHandler
//! [`Event::UserEvent`]: event::Event::UserEvent
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
