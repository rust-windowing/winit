//! Various wrappers around [`CFRunLoop`][objc2_core_foundation::CFRunLoop].
//!
//! See Apple's documentation on Run Loops for details:
//! <https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/RunLoopManagement/RunLoopManagement.html>

mod event_loop_proxy;
mod main_run_loop;
mod main_run_loop_observer;
mod tracing_observers;

pub use self::event_loop_proxy::*;
pub use self::main_run_loop::*;
pub use self::main_run_loop_observer::*;
pub use self::tracing_observers::*;
