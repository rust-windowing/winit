//! Winit is a cross-platform window creation and event loop management library.
//!
//! # Building windows
//!
//! Before you can build a [`Window`], you first need to build an [`EventLoop`]. This is done with the
//! [`EventLoop::new()`] function.
//!
//! ```no_run
//! use winit::event_loop::EventLoop;
//! let event_loop = EventLoop::new();
//! ```
//!
//! Once this is done there are two ways to create a [`Window`]:
//!
//!  - Calling [`Window::new(&event_loop)`][window_new].
//!  - Calling [`let builder = WindowBuilder::new()`][window_builder_new] then [`builder.build(&event_loop)`][window_builder_build].
//!
//! The first method is the simplest, and will give you default values for everything. The second
//! method allows you to customize the way your [`Window`] will look and behave by modifying the
//! fields of the [`WindowBuilder`] object before you create the [`Window`].
//!
//! # Event handling
//!
//! Once a [`Window`] has been created, it will generate different *events*. A [`Window`] object can
//! generate [`WindowEvent`]s when certain input events occur, such as a cursor moving over the
//! window or a key getting pressed while the window is focused. Devices can generate
//! [`DeviceEvent`]s, which contain unfiltered event data that isn't specific to a certain window.
//! Some user activity, like mouse movement, can generate both a [`WindowEvent`] *and* a
//! [`DeviceEvent`]. You can also create and handle your own custom [`UserEvent`]s, if desired.
//!
//! You can retrieve events by calling [`EventLoop::run`][event_loop_run]. This function will
//! dispatch events for every [`Window`] that was created with that particular [`EventLoop`], and
//! will run until the `control_flow` argument given to the closure is set to
//! [`ControlFlow`]`::`[`ExitWithCode`] (which [`ControlFlow`]`::`[`Exit`] aliases to), at which
//! point [`Event`]`::`[`LoopDestroyed`] is emitted and the entire program terminates.
//!
//! Winit no longer uses a `EventLoop::poll_events() -> impl Iterator<Event>`-based event loop
//! model, since that can't be implemented properly on some platforms (e.g web, iOS) and works poorly on
//! most other platforms. However, this model can be re-implemented to an extent with
//! [`EventLoopExtRunReturn::run_return`]. See that method's documentation for more reasons about why
//! it's discouraged, beyond compatibility reasons.
//!
//!
//! ```no_run
//! use winit::{
//!     event::{Event, WindowEvent},
//!     event_loop::EventLoop,
//!     window::WindowBuilder,
//! };
//!
//! let event_loop = EventLoop::new();
//! let window = WindowBuilder::new().build(&event_loop).unwrap();
//!
//! event_loop.run(move |event, _, control_flow| {
//!     // ControlFlow::Poll continuously runs the event loop, even if the OS hasn't
//!     // dispatched any events. This is ideal for games and similar applications.
//!     control_flow.set_poll();
//!
//!     // ControlFlow::Wait pauses the event loop if no events are available to process.
//!     // This is ideal for non-game applications that only update in response to user
//!     // input, and uses significantly less power/CPU time than ControlFlow::Poll.
//!     control_flow.set_wait();
//!
//!     match event {
//!         Event::WindowEvent {
//!             event: WindowEvent::CloseRequested,
//!             ..
//!         } => {
//!             println!("The close button was pressed; stopping");
//!             control_flow.set_exit();
//!         },
//!         Event::MainEventsCleared => {
//!             // Application update code.
//!
//!             // Queue a RedrawRequested event.
//!             //
//!             // You only need to call this if you've determined that you need to redraw, in
//!             // applications which do not always need to. Applications that redraw continuously
//!             // can just render here instead.
//!             window.request_redraw();
//!         },
//!         Event::RedrawRequested(_) => {
//!             // Redraw the application.
//!             //
//!             // It's preferable for applications that do not render continuously to render in
//!             // this event rather than in MainEventsCleared, since rendering in here allows
//!             // the program to gracefully handle redraws requested by the OS.
//!         },
//!         _ => ()
//!     }
//! });
//! ```
//!
//! [`Event`]`::`[`WindowEvent`] has a [`WindowId`] member. In multi-window environments, it should be
//! compared to the value returned by [`Window::id()`][window_id_fn] to determine which [`Window`]
//! dispatched the event.
//!
//! # Drawing on the window
//!
//! Winit doesn't directly provide any methods for drawing on a [`Window`]. However it allows you to
//! retrieve the raw handle of the window and display (see the [`platform`] module and/or the
//! [`raw_window_handle`] and [`raw_display_handle`] methods), which in turn allows
//!  you to create an OpenGL/Vulkan/DirectX/Metal/etc. context that can be used to render graphics.
//!
//! Note that many platforms will display garbage data in the window's client area if the
//! application doesn't render anything to the window by the time the desktop compositor is ready to
//! display the window to the user. If you notice this happening, you should create the window with
//! [`visible` set to `false`](crate::window::WindowBuilder::with_visible) and explicitly make the
//! window visible only once you're ready to render into it.
//!
//! [`EventLoop`]: event_loop::EventLoop
//! [`EventLoopExtRunReturn::run_return`]: ./platform/run_return/trait.EventLoopExtRunReturn.html#tymethod.run_return
//! [`EventLoop::new()`]: event_loop::EventLoop::new
//! [event_loop_run]: event_loop::EventLoop::run
//! [`ControlFlow`]: event_loop::ControlFlow
//! [`Exit`]: event_loop::ControlFlow::Exit
//! [`ExitWithCode`]: event_loop::ControlFlow::ExitWithCode
//! [`Window`]: window::Window
//! [`WindowId`]: window::WindowId
//! [`WindowBuilder`]: window::WindowBuilder
//! [window_new]: window::Window::new
//! [window_builder_new]: window::WindowBuilder::new
//! [window_builder_build]: window::WindowBuilder::build
//! [window_id_fn]: window::Window::id
//! [`Event`]: event::Event
//! [`WindowEvent`]: event::WindowEvent
//! [`DeviceEvent`]: event::DeviceEvent
//! [`UserEvent`]: event::Event::UserEvent
//! [`LoopDestroyed`]: event::Event::LoopDestroyed
//! [`platform`]: platform
//! [`raw_window_handle`]: ./window/struct.Window.html#method.raw_window_handle
//! [`raw_display_handle`]: ./window/struct.Window.html#method.raw_display_handle

#![deny(rust_2018_idioms)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(clippy::all)]
#![cfg_attr(feature = "cargo-clippy", deny(warnings))]
// Doc feature labels can be tested locally by running RUSTDOCFLAGS="--cfg=docsrs" cargo +nightly doc
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![allow(clippy::missing_safety_doc)]

#[allow(unused_imports)]
#[macro_use]
extern crate log;
#[cfg(feature = "serde")]
#[macro_use]
extern crate serde;
#[macro_use]
extern crate bitflags;

pub mod dpi;
#[macro_use]
pub mod error;
pub mod event;
pub mod event_loop;
mod icon;
pub mod monitor;
mod platform_impl;
pub mod window;

pub mod platform;
