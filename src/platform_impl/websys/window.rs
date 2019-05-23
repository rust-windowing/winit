use window::{WindowAttributes, CreationError, MouseCursor};
use std::collections::VecDeque;
use std::rc::Rc;
use std::cell::RefCell;
use dpi::{PhysicalPosition, LogicalPosition, PhysicalSize, LogicalSize};
use icon::Icon;
use super::event_loop::{EventLoopWindowTarget};

use ::wasm_bindgen::prelude::*;
use ::wasm_bindgen::JsCast;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(u32);

impl DeviceId {
    pub fn dummy() -> Self {
        DeviceId(0)
    }
}

///
/// ElementSelection allows the window creator
/// to select an existing canvas in the DOM
/// or a container in which to create a canvas.
///
#[derive(Clone)]
pub enum ElementSelection {
    CanvasId(String),
    ContainerId(String)
}

impl Default for ElementSelection {
    fn default() -> Self { ElementSelection::CanvasId("".to_string()) }
}

///
/// Platform specific attributes for window creation.
/// 
#[derive(Clone, Default)]
pub struct PlatformSpecificWindowBuilderAttributes {
    pub element: ElementSelection
}

#[derive(Copy, Clone, Debug)]
pub struct MonitorHandle;

impl MonitorHandle {
    /// Returns a human-readable name of the monitor.
    ///
    /// Returns `None` if the monitor doesn't exist anymore.
    #[inline]
    pub fn get_name(&self) -> Option<String> {
        unimplemented!()
    }

    /// Returns the monitor's resolution.
    #[inline]
    pub fn get_dimensions(&self) -> PhysicalSize {
        unimplemented!()
    }

    /// Returns the top-left corner position of the monitor relative to the larger full
    /// screen area.
    #[inline]
    pub fn get_position(&self) -> PhysicalPosition {
        unimplemented!()
    }

    /// Returns the DPI factor that can be used to map logical pixels to physical pixels, and vice versa.
    ///
    /// See the [`dpi`](dpi/index.html) module for more information.
    ///
    /// ## Platform-specific
    ///
    /// - **X11:** Can be overridden using the `WINIT_HIDPI_FACTOR` environment variable.
    /// - **Android:** Always returns 1.0.
    #[inline]
    pub fn get_hidpi_factor(&self) -> f64 {
        unimplemented!()
    }
}

pub struct Window {
    pub(crate) canvas: ::web_sys::HtmlCanvasElement,
}

pub(crate) struct WindowInternal<'a, T: 'static> {
    pub target: &'a EventLoopWindowTarget<T>,
    _marker: std::marker::PhantomData<T>,
}

macro_rules! install_simple_handler {
    ($in:ty, $f:ident, $w:ident, $e:ident) => {
        let win = $w.clone();
        let handler = Closure::wrap(Box::new(move |event: $in| {
            win.target.window_events.borrow_mut().push(event.into());
            if win.target.is_sleeping() {
                win.target.wake();
            }
        }) as Box<FnMut($in)>);
        $e.$f(Some(handler.as_ref().unchecked_ref()));
        handler.forget();
    }
}

impl Window {
    /// Creates a new Window for platforms where this is appropriate.
    ///
    /// This function is equivalent to `WindowBuilder::new().build(event_loop)`.
    ///
    /// Error should be very rare and only occur in case of permission denied, incompatible system,
    ///  out of memory, etc.
    #[inline]
    pub fn new<T: 'static>(target: &EventLoopWindowTarget<T>, 
                           attr: WindowAttributes,
                           ps_attr: PlatformSpecificWindowBuilderAttributes) 
                           -> Result<Window, CreationError> {
        let window = ::web_sys::window()
            .expect("No global window object found!");
        let document = window.document()
            .expect("Global window does not have a document!");

        let element = match ps_attr.element {
            ElementSelection::CanvasId(id) => {
                document.get_element_by_id(&id)
                    .expect(&format!("No canvas with ID {} found", id))
                    .dyn_into::<::web_sys::HtmlCanvasElement>().unwrap()
            },
            ElementSelection::ContainerId(id) => {
                let parent = document.get_element_by_id(&id)
                    .expect(&format!("No container element with Id {} found", id));
                
                let canvas = document.create_element("canvas")
                    .expect("Could not create a canvas")
                    .dyn_into::<::web_sys::HtmlCanvasElement>().unwrap();
                
                parent.append_child(&canvas);

                canvas
            }
        };

        let internal = Rc::new(WindowInternal {
            target: &target,
            _marker: std::marker::PhantomData,
        });

        target.setup_window(&element);

        // TODO: move these to event_loop.  we can pass 'static closures to web_sys::Closure,
        // meaning i can't use the reference to target.  womp womp.
        //
        // we should still own the canvas, but events should be with the event loop.
        // should bring back target.set_window, which can register the event handlers.
        /*
        install_simple_handler!(::web_sys::MouseEvent, set_onmousedown, internal, element);
        install_simple_handler!(::web_sys::MouseEvent, set_onmouseup, internal, element);
        install_simple_handler!(::web_sys::MouseEvent, set_onmouseenter, internal, element);
        install_simple_handler!(::web_sys::MouseEvent, set_onmouseleave, internal, element);
        */


        Ok(Window {
            canvas: element,
        })
    }

    /// Modifies the title of the window.
    ///
    /// This is a no-op if the window has already been closed.
    #[inline]
    pub fn set_title(&self, title: &str) {
    }

    /// Shows the window if it was hidden.
    ///
    /// ## Platform-specific
    ///
    /// - Has no effect on Android
    ///
    #[inline]
    pub fn show(&self) {
        unimplemented!()
    }

    /// Hides the window if it was visible.
    ///
    /// ## Platform-specific
    ///
    /// - Has no effect on Android
    ///
    #[inline]
    pub fn hide(&self) {
        unimplemented!()
    }

    /// Emits a `WindowEvent::RedrawRequested` event in the associated event loop after all OS
    /// events have been processed by the event loop.
    ///
    /// This is the **strongly encouraged** method of redrawing windows, as it can integrates with
    /// OS-requested redraws (e.g. when a window gets resized).
    ///
    /// This function can cause `RedrawRequested` events to be emitted after `Event::EventsCleared`
    /// but before `Event::NewEvents` if called in the following circumstances:
    /// * While processing `EventsCleared`.
    /// * While processing a `RedrawRequested` event that was sent during `EventsCleared` or any
    ///   directly subsequent `RedrawRequested` event.
    pub fn request_redraw(&self) {
        unimplemented!()
    }

    /// Returns the position of the top-left hand corner of the window relative to the
    ///  top-left hand corner of the desktop.
    ///
    /// Note that the top-left hand corner of the desktop is not necessarily the same as
    ///  the screen. If the user uses a desktop with multiple monitors, the top-left hand corner
    ///  of the desktop is the top-left hand corner of the monitor at the top-left of the desktop.
    ///
    /// The coordinates can be negative if the top-left hand corner of the window is outside
    ///  of the visible screen region.
    ///
    /// Returns `None` if the window no longer exists.
    #[inline]
    pub fn get_position(&self) -> Option<LogicalPosition> {
        unimplemented!()
    }

    /// Returns the position of the top-left hand corner of the window's client area relative to the
    /// top-left hand corner of the desktop.
    ///
    /// The same conditions that apply to `get_position` apply to this method.
    #[inline]
    pub fn get_inner_position(&self) -> Option<LogicalPosition> {
        // websys: we have no concept of "inner" client area, so just return position.
        self.get_position()
    }

    /// Modifies the position of the window.
    ///
    /// See `get_position` for more information about the coordinates.
    ///
    /// This is a no-op if the window has already been closed.
    #[inline]
    pub fn set_position(&self, position: LogicalPosition) {
        unimplemented!()
    }

    /// Returns the logical size of the window's client area.
    ///
    /// The client area is the content of the window, excluding the title bar and borders.
    ///
    /// Converting the returned `LogicalSize` to `PhysicalSize` produces the size your framebuffer should be.
    ///
    /// Returns `None` if the window no longer exists.
    #[inline]
    pub fn get_inner_size(&self) -> Option<LogicalSize> {
        // websys: we have no concept of "inner" client area, so just return size.
        self.get_outer_size()
    }

    /// Returns the logical size of the entire window.
    ///
    /// These dimensions include the title bar and borders. If you don't want that (and you usually don't),
    /// use `get_inner_size` instead.
    ///
    /// Returns `None` if the window no longer exists.
    #[inline]
    pub fn get_outer_size(&self) -> Option<LogicalSize> {
        unimplemented!()
    }

    /// Modifies the inner size of the window.
    ///
    /// See `get_inner_size` for more information about the values.
    ///
    /// This is a no-op if the window has already been closed.
    #[inline]
    pub fn set_inner_size(&self, size: LogicalSize) {
        unimplemented!()
    }

    /// Sets a minimum dimension size for the window.
    #[inline]
    pub fn set_min_dimensions(&self, dimensions: Option<LogicalSize>) {
        unimplemented!()
    }

    /// Sets a maximum dimension size for the window.
    #[inline]
    pub fn set_max_dimensions(&self, dimensions: Option<LogicalSize>) {
        unimplemented!()
    }

    /// Sets whether the window is resizable or not.
    ///
    /// Note that making the window unresizable doesn't exempt you from handling `Resized`, as that event can still be
    /// triggered by DPI scaling, entering fullscreen mode, etc.
    ///
    /// ## Platform-specific
    ///
    /// This only has an effect on desktop platforms.
    ///
    /// Due to a bug in XFCE, this has no effect on Xfwm.
    #[inline]
    pub fn set_resizable(&self, resizable: bool) {
        unimplemented!()
    }

    /// Returns the DPI factor that can be used to map logical pixels to physical pixels, and vice versa.
    ///
    /// See the [`dpi`](dpi/index.html) module for more information.
    ///
    /// Note that this value can change depending on user action (for example if the window is
    /// moved to another screen); as such, tracking `WindowEvent::HiDpiFactorChanged` events is
    /// the most robust way to track the DPI you need to use to draw.
    ///
    /// ## Platform-specific
    ///
    /// - **X11:** This respects Xft.dpi, and can be overridden using the `WINIT_HIDPI_FACTOR` environment variable.
    /// - **Android:** Always returns 1.0.
    #[inline]
    pub fn get_hidpi_factor(&self) -> f64 {
        1.0
    }

    /// Modifies the mouse cursor of the window.
    /// Has no effect on Android.
    #[inline]
    pub fn set_cursor(&self, cursor: MouseCursor) {
        unimplemented!()
    }

    /// Changes the position of the cursor in window coordinates.
    #[inline]
    pub fn set_cursor_position(&self, position: LogicalPosition) -> Result<(), String> {
        unimplemented!()
    }

    /// Grabs the cursor, preventing it from leaving the window.
    ///
    /// ## Platform-specific
    ///
    /// On macOS, this presently merely locks the cursor in a fixed location, which looks visually awkward.
    ///
    /// This has no effect on Android or iOS.
    #[inline]
    pub fn grab_cursor(&self, grab: bool) -> Result<(), String> {
        unimplemented!()
    }

    /// Hides the cursor, making it invisible but still usable.
    ///
    /// ## Platform-specific
    ///
    /// On Windows and X11, the cursor is only hidden within the confines of the window.
    ///
    /// On macOS, the cursor is hidden as long as the window has input focus, even if the cursor is outside of the
    /// window.
    ///
    /// This has no effect on Android or iOS.
    #[inline]
    pub fn hide_cursor(&self, hide: bool) {
        unimplemented!()
    }

    /// Sets the window to maximized or back
    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        unimplemented!()
    }

    /// Sets the window to fullscreen or back
    #[inline]
    pub fn set_fullscreen(&self, monitor: Option<::monitor::MonitorHandle>) {
        unimplemented!()
    }

    /// Turn window decorations on or off.
    #[inline]
    pub fn set_decorations(&self, decorations: bool) {
        unimplemented!()
    }

    /// Change whether or not the window will always be on top of other windows.
    #[inline]
    pub fn set_always_on_top(&self, always_on_top: bool) {
        unimplemented!()
    }

    /// Sets the window icon. On Windows and X11, this is typically the small icon in the top-left
    /// corner of the titlebar.
    ///
    /// For more usage notes, see `WindowBuilder::with_window_icon`.
    ///
    /// ## Platform-specific
    ///
    /// This only has an effect on Windows and X11.
    #[inline]
    pub fn set_window_icon(&self, window_icon: Option<Icon>) {
        unimplemented!()
    }

    /// Sets location of IME candidate box in client area coordinates relative to the top left.
    #[inline]
    pub fn set_ime_spot(&self, position: LogicalPosition) {
        unimplemented!()
    }

    /// Returns the monitor on which the window currently resides
    #[inline]
    pub fn get_current_monitor(&self) -> ::monitor::MonitorHandle {
        ::monitor::MonitorHandle{inner: MonitorHandle{}}
    }

    /// Returns the list of all the monitors available on the system.
    ///
    /// This is the same as `EventLoop::get_available_monitors`, and is provided for convenience.
    #[inline]
    pub fn get_available_monitors(&self) -> VecDeque<MonitorHandle> {
        unimplemented!()
    }

    /// Returns the primary monitor of the system.
    ///
    /// This is the same as `EventLoop::get_primary_monitor`, and is provided for convenience.
    #[inline]
    pub fn get_primary_monitor(&self) -> MonitorHandle {
        MonitorHandle {}
    }

    /// Returns an identifier unique to the window.
    #[inline]
    pub fn id(&self) -> WindowId {
        unimplemented!()
    }

}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId {
}


impl WindowId {
    /// Returns a dummy `WindowId`, useful for unit testing. The only guarantee made about the return
    /// value of this function is that it will always be equal to itself and to future values returned
    /// by this function.  No other guarantees are made. This may be equal to a real `WindowId`.
    ///
    /// **Passing this into a winit function will result in undefined behavior.**
    pub unsafe fn dummy() -> Self {
        WindowId{}
    }
}
