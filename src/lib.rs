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
//! Once a [`Window`] has been created, it will *generate events*. For example whenever the user moves
//! the [`Window`], resizes the [`Window`], moves the mouse, etc. an event is generated.
//!
//! The events generated by a [`Window`] can be retreived from the [`EventLoop`] the [`Window`] was created
//! with.
//!
//! You do this by calling [`event_loop.run(...)`][event_loop_run]. This function will run forever
//! unless `control_flow` is set to [`ControlFlow`]`::`[`Exit`], at which point [`Event`]`::`[`LoopDestroyed`]
//! is emitted and the entire program terminates.
//!
//! ```no_run
//! use winit::event_loop::ControlFlow;
//! use winit::event::{Event, WindowEvent};
//! # use winit::event_loop::EventLoop;
//! # let event_loop = EventLoop::new();
//!
//! event_loop.run(move |event, _, control_flow| {
//!     match event {
//!         Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
//!             println!("The close button was pressed; stopping");
//!             *control_flow = ControlFlow::Exit
//!         },
//!         _ => *control_flow = ControlFlow::Wait,
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
//! [`EventLoop`]: ./event_loop/struct.EventLoop.html
//! [`EventLoop::new()`]: ./event_loop/struct.EventLoop.html#method.new
//! [event_loop_run]: ./event_loop/struct.EventLoop.html#method.run
//! [`ControlFlow`]: ./event_loop/enum.ControlFlow.html
//! [`Exit`]: ./event_loop/enum.ControlFlow.html#variant.Exit
//! [`Window`]: ./window/struct.Window.html
//! [`WindowBuilder`]: ./window/struct.WindowBuilder.html
//! [window_new]: ./window/struct.Window.html#method.new
//! [window_builder_new]: ./window/struct.WindowBuilder.html#method.new
//! [window_builder_build]: ./window/struct.WindowBuilder.html#method.build
//! [window_id_fn]: ./window/struct.Window.html#method.id
//! [`Event`]: ./event/enum.Event.html
//! [`WindowEvent`]: ./event/enum.Event.html#variant.WindowEvent
//! [`LoopDestroyed`]: ./event/enum.Event.html#variant.LoopDestroyed
//! [`platform`]: ./platform/index.html

#[allow(unused_imports)]
#[macro_use]
extern crate lazy_static;
extern crate libc;
#[macro_use]
extern crate log;
#[cfg(feature = "serde")]
#[macro_use]
extern crate serde;

#[cfg(target_os = "windows")]
extern crate winapi;
#[cfg(target_os = "windows")]
extern crate backtrace;
#[macro_use]
#[cfg(target_os = "windows")]
extern crate bitflags;
#[cfg(any(target_os = "macos", target_os = "ios"))]
#[macro_use]
extern crate objc;
#[cfg(target_os = "macos")]
extern crate cocoa;
#[cfg(target_os = "macos")]
extern crate dispatch;
#[cfg(target_os = "macos")]
extern crate core_foundation;
#[cfg(target_os = "macos")]
extern crate core_graphics;
#[cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd"))]
extern crate x11_dl;
#[cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd", target_os = "windows"))]
extern crate parking_lot;
#[cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd"))]
extern crate percent_encoding;
#[cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd"))]
extern crate smithay_client_toolkit as sctk;
#[cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd"))]
extern crate calloop;

pub mod dpi;
#[macro_use]
pub mod error;
pub mod event;
pub mod event_loop;
mod icon;
mod platform_impl;
pub mod window;
pub mod monitor;

pub mod platform;
