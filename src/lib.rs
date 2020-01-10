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
//! You can retreive events by calling [`EventLoop::run`][event_loop_run]. This function will
//! dispatch events for every [`Window`] that was created with that particular [`EventLoop`], and
//! will run until the `control_flow` argument given to the closure is set to
//! [`ControlFlow`]`::`[`Exit`], at which point [`Event`]`::`[`LoopDestroyed`] is emitted and the
//! entire program terminates.
//!
//! Winit no longer uses a `EventLoop::poll_events() -> impl Iterator<Event>`-based event loop
//! model, since that can't be implemented properly on web and mobile platforms and works poorly on
//! most desktop platforms. However, this model can be re-implemented to an extent on desktops with
//! [`EventLoopExtDesktop::run_return`]. See that method's documentation for more reasons about why
//! it's discouraged, beyond mobile/web compatibility reasons.
//!
//!
//! ```no_run
//! use winit::{
//!     event::{Event, WindowEvent},
//!     event_loop::{ControlFlow, EventLoop},
//!     window::WindowBuilder,
//! };
//!
//! let event_loop = EventLoop::new();
//! let window = WindowBuilder::new().build(&event_loop).unwrap();
//!
//! event_loop.run(move |event, _, control_flow| {
//!     // ControlFlow::Poll continuously runs the event loop, even if the OS hasn't
//!     // dispatched any events. This is ideal for games and similar applications.
//!     *control_flow = ControlFlow::Poll;
//!
//!     // ControlFlow::Wait pauses the event loop if no events are available to process.
//!     // This is ideal for non-game applications that only update in response to user
//!     // input, and uses significantly less power/CPU time than ControlFlow::Poll.
//!     *control_flow = ControlFlow::Wait;
//!
//!     match event {
//!         Event::WindowEvent {
//!             event: WindowEvent::CloseRequested,
//!             ..
//!         } => {
//!             println!("The close button was pressed; stopping");
//!             *control_flow = ControlFlow::Exit
//!         },
//!         Event::MainEventsCleared => {
//!             // Application update code.
//!
//!             // Queue a RedrawRequested event.
//!             window.request_redraw();
//!         },
//!         Event::RedrawRequested(_) => {
//!             // Redraw the application.
//!             //
//!             // It's preferrable to render in this event rather than in MainEventsCleared, since
//!             // rendering in here allows the program to gracefully handle redraws requested
//!             // by the OS.
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
//! retrieve the raw handle of the window (see the [`platform`] module), which in turn allows you
//! to create an OpenGL/Vulkan/DirectX/Metal/etc. context that can be used to render graphics.
//!
//! [`EventLoop`]: event_loop::EventLoop
//! [`EventLoopExtDesktop::run_return`]: ./platform/desktop/trait.EventLoopExtDesktop.html#tymethod.run_return
//! [`EventLoop::new()`]: event_loop::EventLoop::new
//! [event_loop_run]: event_loop::EventLoop::run
//! [`ControlFlow`]: event_loop::ControlFlow
//! [`Exit`]: event_loop::ControlFlow::Exit
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

#![deny(rust_2018_idioms)]
#![deny(intra_doc_link_resolution_failure)]

#[allow(unused_imports)]
#[macro_use]
extern crate lazy_static;
#[allow(unused_imports)]
#[macro_use]
extern crate log;
#[cfg(feature = "serde")]
#[macro_use]
extern crate serde;
#[macro_use]
extern crate bitflags;
#[cfg(any(target_os = "macos", target_os = "ios"))]
#[macro_use]
extern crate objc;
#[cfg(all(target_arch = "wasm32", feature = "std_web"))]
extern crate std_web as stdweb;

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
