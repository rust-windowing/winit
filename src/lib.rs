//! Winit allows you to build a window on as many platforms as possible.
//!
//! # Building a window
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
//! The first way is the simplest way and will give you default values for everything.
//!
//! The second way allows you to customize the way your [`Window`] will look and behave by modifying
//! the fields of the [`WindowBuilder`] object before you create the [`Window`].
//!
//! # Event handling
//!
//! Once a [`Window`] has been created, it will generate different *events*. A [`Window`] object can
//! generate a [`WindowEvent`] when certain things happen, like whenever the user moves their mouse
//! or presses a key inside the [`Window`]. Devices can generate a [`DeviceEvent`] directly as well,
//! which contains unfiltered event data that isn't specific to a certain window. Some user
//! activity, like mouse movement, can generate both a [`WindowEvent`] *and* a [`DeviceEvent`]. You
//! can also create and handle your own custom [`UserEvent`]s, if desired.
//!
//! Events can be retreived by using an [`EventLoop`]. A [`Window`] will send its events to the
//! [`EventLoop`] object it was created with.
//!
//! You do this by calling [`event_loop.run(...)`][event_loop_run]. This function will run forever
//! unless `control_flow` is set to [`ControlFlow`]`::`[`Exit`], at which point [`Event`]`::`[`LoopDestroyed`]
//! is emitted and the entire program terminates.
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
//!     match event {
//!         Event::EventsCleared => {
//!             // Application update code.
//!
//!             // Queue a RedrawRequested event.
//!             window.request_redraw();
//!         },
//!         Event::WindowEvent {
//!             event: WindowEvent::RedrawRequested,
//!             ..
//!         } => {
//!             // Redraw the application.
//!             //
//!             // It's preferrable to render in this event rather than in EventsCleared, since
//!             // rendering in here allows the program to gracefully handle redraws requested
//!             // by the OS.
//!         },
//!         Event::WindowEvent {
//!             event: WindowEvent::CloseRequested,
//!             ..
//!         } => {
//!             println!("The close button was pressed; stopping");
//!             *control_flow = ControlFlow::Exit
//!         },
//!         // ControlFlow::Poll continuously runs the event loop, even if the OS hasn't
//!         // dispatched any events. This is ideal for games and similar applications.
//!         _ => *control_flow = ControlFlow::Poll,
//!         // ControlFlow::Wait pauses the event loop if no events are available to process.
//!         // This is ideal for non-game applications that only update in response to user
//!         // input, and uses significantly less power/CPU time than ControlFlow::Poll.
//!         // _ => *control_flow = ControlFlow::Wait,
//!     }
//! });
//! ```
//!
//! If you use multiple [`Window`]s, [`Event`]`::`[`WindowEvent`] has a member named `window_id`. You can
//! compare it with the value returned by the [`id()`][window_id_fn] method of [`Window`] in order to know which
//! [`Window`] has received the event.
//!
//! # Drawing on the window
//!
//! Winit doesn't provide any function that allows drawing on a [`Window`]. However it allows you to
//! retrieve the raw handle of the window (see the [`platform`] module), which in turn allows you
//! to create an OpenGL/Vulkan/DirectX/Metal/etc. context that will draw on the [`Window`].
//!
//! [`EventLoop`]: event_loop::EventLoop
//! [`EventLoop::new()`]: event_loop::EventLoop::new
//! [event_loop_run]: event_loop::EventLoop::run
//! [`ControlFlow`]: event_loop::ControlFlow
//! [`Exit`]: event_loop::ControlFlow::Exit
//! [`Window`]: window::Window
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
#[macro_use]
extern crate log;
#[cfg(feature = "serde")]
#[macro_use]
extern crate serde;
#[macro_use]
#[cfg(any(target_os = "ios", target_os = "windows"))]
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
