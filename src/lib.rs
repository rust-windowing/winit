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
//! # Overview of the event loop
//!
//! An [`EventLoop`] object must be *run* so that [`Window`]s created with it can be correctly displayed and function.
//! This is done by passing a closure to [`EventLoop::run()`], which will take control of the
//! current process. All application code is then managed from this closure.
//!
//! A running [`EventLoop`] works by passing [`Event`]s to the closure. The closure handles these events and can control
//! the behaviour of the loop by modifying the [`ControlFlow`] that is also passed to the closure. The closure is
//! invoked with each event, the order of which is illustrated by the following diagram, showing how the events loop.
//!
//! <img src="https://raw.githubusercontent.com/rust-windowing/winit/master/docs/event-loop.svg" alt="Flowchart illustrating order of events" height="630px" style="float:left">
//!
//! The progress of the event loop is marked by so-called "loop-stage events". Key application logic is most often run
//! while handling such loop-stage events.
//!
//! Every event loop starts with a [`NewEvents`] event with [`StartCause::Init`], followed by a [`Resumed`] loop-stage
//! event. Application initialization - including initializing graphics resources - should typically be done while handling
//! this first [`Resumed`] event.
//!
//! The primary loop of events always starts with the loop-stage event [`NewEvents`], containing a [`StartCause`].
//! This is then followed by any "main events" generated by the platform. "Main events" include any and all
//! [`WindowEvent`]s, [`DeviceEvent`]s and [`UserEvent`]s, but not any redraw events. The [`MainEventsCleared`] loop-stage event
//! indicates when all main events have been exhausted and handling this is typically where the "main body" of your application
//! should be run, since all input has been handled for this loop iteration. Applications which need to constantly update
//! their graphics (for example, games) will typically call [`request_redraw()`] while handling [`MainEventsCleared`].
//!
//! If the window needs to be redrawn (because either the platform or application code requests it), then a
//! [`RedrawRequested`] event follows. winit will ensure that this is sent at most once per loop so that applications
//! can run rendering code here, confident that work will not be duplicated. This is always followed by the
//! [`RedrawEventsCleared`] loop-stage event, marking the end of the primary loop.
//!
//! Some platforms currently run one iteration of this primary loop immediately after the first [`Resumed`] event, but
//! this is platform-dependent and may change.
//!
//! The [`Suspended`] and [`Resumed`] loop-stage events are used on some platforms (typically mobile platforms) to
//! indicate suspension and resumption of the application by the platform. A `Suspended` event breaks out of the loop,
//! and the next event is guaranteed to be a `Resumed` event. See the [detailed documentation][`Suspended`]
//! for more on these events.
//!
//! Once the [`ControlFlow`] has been set to exit (typically with [`ControlFlow::set_exit()`] or
//! [`set_exit_with_code()`][`ControlFlow::set_exit_with_code()`]) a final [`LoopDestroyed`] event is dispatched. This
//! is irreversible and a typical time to free all application resources, as the process will be terminated after this.
//!
//! This overview should be correct for cross-platform use in most cases, please see the detailed descriptions in the
//! API docs for the exact behaviour of each event and details of any platform-dependent behaviour. In particular on some
//! platforms it is possible (though discouraged) to use [`run_return()`][`EventLoopExtRunReturn::run_return`] to prevent
//! [`run()`][`EventLoop::run()`] from hijacking the process, however this may change the order of events significantly,
//! so carefully check the API documentation.
//!
//! # Using the event loop
//!
//! As [`EventLoop::run()`] takes control of the process - and will terminate the process before returning - all application
//! code is managed from the closure passed in. For example:
//!
//! ```no_run
//! use winit::{
//!     event::{Event, WindowEvent},
//!     event_loop::EventLoop,
//!     window::WindowBuilder,
//! };
//!
//! struct MyApp {
//!     frame_count: usize,
//! }
//!
//! let event_loop = EventLoop::new();
//! let window = WindowBuilder::new().build(&event_loop).unwrap();
//! // Data structures that can access application data must be moved into the
//! // closure passed to `run`.
//! let mut app = MyApp { frame_count: 0 };
//!
//! // Note the `move` closure: the app object is now owned by the closure.
//! event_loop.run(move |event, _window_target, control_flow| {
//!     // ControlFlow::Wait pauses the event loop until events are available to process.
//!     // This is ideal for non-game applications that only update in response to user
//!     // input, and uses significantly less power/CPU time than ControlFlow::Poll.
//!     control_flow.set_wait();
//!
//!     // ControlFlow::Poll continuously runs the event loop, even if the platform hasn't
//!     // dispatched any events. This is ideal for games and similar applications which
//!     // require a constant stream of redraw events.
//!     control_flow.set_poll();
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
//!             // Since we have `ControlFlow::Poll` and request a redraw every `MainEventsCleared`,
//!             // then this will be run exactly once per rendered frame.
//!             app.frame_count += 1;
//!             println!("Frame: {}", app.frame_count);
//!
//!             // Queue a RedrawRequested event.
//!             //
//!             // You only need to call this if you've determined that you need to redraw, in
//!             // applications which do not always need to.
//!             window.request_redraw();
//!         },
//!         Event::RedrawRequested(_) => {
//!             // Redraw the application.
//!             //
//!             // It's preferable for applications that do not render continuously to render in
//!             // this event rather than in MainEventsCleared, since rendering in here allows
//!             // the program to gracefully handle redraws requested by the platform.
//!         },
//!         _ => ()
//!     }
//! });
//! ```
//!
//! [`Event::WindowEvent`][event::Event::WindowEvent] has a [`WindowId`] member. In multi-window environments, it should be
//! compared to the value returned by [`Window::id()`][window_id_fn] to determine which [`Window`]
//! dispatched the event.
//!
//! Applications that need to create new windows while an event loop is currently running can do so using
//! the [`EventLoopWindowTarget`] passed to the event handler closure.
//!
//! # Drawing on the window
//!
//! Winit doesn't directly provide any methods for drawing on a [`Window`]. However it allows you to
//! retrieve the raw handle of the window and display (see the [`platform`] module and/or the
//! [`raw_window_handle`] and [`raw_display_handle`] methods), which in turn allows
//!  you to create an OpenGL/Vulkan/DirectX/Metal/etc. context that can be used to render graphics.
//!
//! Note that several platforms will display garbage data in the window's client area if the
//! application doesn't render anything to the window by the time the desktop compositor is ready to
//! display the window to the user. If you notice this happening, you should create the window with
//! [`visible` set to `false`](crate::window::WindowBuilder::with_visible) and explicitly make the
//! window visible only once you're ready to render into it.
//!
//! [`EventLoop`]: event_loop::EventLoop
//! [`EventLoopExtRunReturn::run_return`]: ./platform/run_return/trait.EventLoopExtRunReturn.html#tymethod.run_return
//! [`EventLoop::new()`]: event_loop::EventLoop::new
//! [`EventLoop::run()`]: event_loop::EventLoop::run
//! [`EventLoopWindowTarget`]: event_loop::EventLoopWindowTarget
//! [`ControlFlow`]: event_loop::ControlFlow
//! [`ControlFlow::set_exit()`]: event_loop::ControlFlow::set_exit
//! [`ControlFlow::set_exit_with_code()`]: event_loop::ControlFlow::set_exit_with_code
//! [`Exit`]: event_loop::ControlFlow::Exit
//! [`ExitWithCode`]: event_loop::ControlFlow::ExitWithCode
//! [`Window`]: window::Window
//! [`request_redraw()`]: window::Window::request_redraw
//! [`WindowId`]: window::WindowId
//! [`WindowBuilder`]: window::WindowBuilder
//! [window_new]: window::Window::new
//! [window_builder_new]: window::WindowBuilder::new
//! [window_builder_build]: window::WindowBuilder::build
//! [window_id_fn]: window::Window::id
//! [`Event`]: event::Event
//! [`NewEvents`]: event::Event::NewEvents
//! [`Resumed`]: event::Event::Resumed
//! [`WindowEvent`]: event::WindowEvent
//! [`DeviceEvent`]: event::DeviceEvent
//! [`UserEvent`]: event::Event::UserEvent
//! [`MainEventsCleared`]: event::Event::MainEventsCleared
//! [`RedrawRequested`]: event::Event::RedrawRequested
//! [`RedrawEventsCleared`]: event::Event::RedrawEventsCleared
//! [`Suspended`]: event::Event::Suspended
//! [`LoopDestroyed`]: event::Event::LoopDestroyed
//! [`StartCause`]: event::StartCause
//! [`StartCause::Init`]: event::StartCause::Init
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
