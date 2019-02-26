use dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use event::{Event, StartCause};
use event_loop::{ControlFlow, EventLoopWindowTarget as RootELW, EventLoopClosed};
use icon::Icon;
use monitor::{MonitorHandle as RootMH};
use window::{CreationError, MouseCursor, WindowAttributes};
use stdweb::{
    traits::*,
    web::{
        document,
        event::*,
        html_element::CanvasElement,
    }
};
use std::cell::{RefCell, RefMut};
use std::collections::VecDeque;
use std::collections::vec_deque::IntoIter as VecDequeIter;
use std::marker::PhantomData;
use std::rc::Rc;

// TODO: dpi
// TODO: pointer locking (stdweb PR required)
// TODO: should there be a maximization / fullscreen API?

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId;

impl DeviceId {
    pub unsafe fn dummy() -> Self {
        DeviceId
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MonitorHandle;

impl MonitorHandle {
    pub fn get_hidpi_factor(&self) -> f64 {
        unimplemented!();
    }

    pub fn get_position(&self) -> PhysicalPosition {
        unimplemented!();
    }

    pub fn get_dimensions(&self) -> PhysicalSize {
        unimplemented!();
    }

    pub fn get_name(&self) -> Option<String> {
        unimplemented!();
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId;

impl WindowId {
    pub unsafe fn dummy() -> WindowId {
        WindowId
    }
}

pub struct Window {
    canvas: CanvasElement,
}

impl Window {
    // TODO: type of window_target
    pub fn new<T>(target: &EventLoopWindowTarget<T>, window: WindowAttributes, platform: PlatformSpecificWindowBuilderAttributes) -> Result<Self, CreationError> {
        unimplemented!();
    }

    pub fn set_title(&self, title: &str) {
        document().set_title(title);
    }

    pub fn show(&self) {
        // Intentionally a no-op
    }

    pub fn hide(&self) {
        // Intentionally a no-op
    }

    pub fn request_redraw(&self) {
        // TODO: what does this mean
        unimplemented!();
    }

    pub fn get_position(&self) -> Option<LogicalPosition> {
        let bounds = self.canvas.get_bounding_client_rect();
        Some(LogicalPosition {
            x: bounds.get_x(),
            y: bounds.get_y(),
        })
    }

    pub fn get_inner_position(&self) -> Option<LogicalPosition> {
        self.get_inner_position()
    }

    pub fn set_position(&self, position: LogicalPosition) {
        // TODO: use CSS?
        unimplemented!();
    }

    #[inline]
    pub fn get_inner_size(&self) -> Option<LogicalSize> {
        Some(LogicalSize {
            width: self.canvas.width() as f64,
            height: self.canvas.height() as f64
        })
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<LogicalSize> {
        Some(LogicalSize {
            width: self.canvas.width() as f64,
            height: self.canvas.height() as f64
        })
    }

    #[inline]
    pub fn set_inner_size(&self, size: LogicalSize) {
        self.canvas.set_width(size.width as u32);
        self.canvas.set_height(size.height as u32);
    }

    #[inline]
    pub fn set_min_dimensions(&self, _dimensions: Option<LogicalSize>) {
        // Intentionally a no-op: users can't resize canvas elements
    }

    #[inline]
    pub fn set_max_dimensions(&self, _dimensions: Option<LogicalSize>) {
        // Intentionally a no-op: users can't resize canvas elements
    }

    #[inline]
    pub fn set_resizable(&self, _resizable: bool) {
        // Intentionally a no-op: users can't resize canvas elements
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f64 {
        unimplemented!();
    }

    #[inline]
    pub fn set_cursor(&self, cursor: MouseCursor) {
        let text = match cursor {
            MouseCursor::Default => "auto",
            MouseCursor::Crosshair => "crosshair",
            MouseCursor::Hand => "pointer",
            MouseCursor::Arrow => "default",
            MouseCursor::Move => "move",
            MouseCursor::Text => "text",
            MouseCursor::Wait => "wait",
            MouseCursor::Help => "help",
            MouseCursor::Progress => "progress",

            MouseCursor::NotAllowed => "not-allowed",
            MouseCursor::ContextMenu => "context-menu",
            MouseCursor::Cell => "cell",
            MouseCursor::VerticalText => "vertical-text",
            MouseCursor::Alias => "alias",
            MouseCursor::Copy => "copy",
            MouseCursor::NoDrop => "no-drop",
            MouseCursor::Grab => "grab",
            MouseCursor::Grabbing => "grabbing",
            MouseCursor::AllScroll => "all-scroll",
            MouseCursor::ZoomIn => "zoom-in",
            MouseCursor::ZoomOut => "zoom-out",

            MouseCursor::EResize => "e-resize",
            MouseCursor::NResize => "n-resize",
            MouseCursor::NeResize => "ne-resize",
            MouseCursor::NwResize => "nw-resize",
            MouseCursor::SResize => "s-resize",
            MouseCursor::SeResize => "se-resize",
            MouseCursor::SwResize => "sw-resize",
            MouseCursor::WResize => "w-resize",
            MouseCursor::EwResize => "ew-resize",
            MouseCursor::NsResize => "ns-resize",
            MouseCursor::NeswResize => "nesw-resize",
            MouseCursor::NwseResize => "nwse-resize",
            MouseCursor::ColResize => "col-resize",
            MouseCursor::RowResize => "row-resize",
        };
        self.canvas.set_attribute("cursor", text)
            .expect("Setting the cursor on the canvas");
    }

    #[inline]
    pub fn set_cursor_position(&self, position: LogicalPosition) -> Result<(), String> {
        // TODO: pointer capture
        unimplemented!();
    }

    #[inline]
    pub fn grab_cursor(&self, grab: bool) -> Result<(), String> {
        // TODO: pointer capture
        unimplemented!();
    }

    #[inline]
    pub fn hide_cursor(&self, hide: bool) {
        self.canvas.set_attribute("cursor", "none")
            .expect("Setting the cursor on the canvas");
    }

    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        // TODO: should there be a maximization / fullscreen API?
        unimplemented!();
    }

    #[inline]
    pub fn set_fullscreen(&self, monitor: Option<RootMH>) {
        // TODO: should there be a maximization / fullscreen API?
        unimplemented!();
    }

    #[inline]
    pub fn set_decorations(&self, _decorations: bool) {
        // Intentionally a no-op, no canvas decorations
    }

    #[inline]
    pub fn set_always_on_top(&self, _always_on_top: bool) {
        // Intentionally a no-op, no window ordering
    }

    #[inline]
    pub fn set_window_icon(&self, window_icon: Option<Icon>) {
        // TODO: should this set the favicon?
        unimplemented!();
    }

    #[inline]
    pub fn set_ime_spot(&self, position: LogicalPosition) {
        unimplemented!();
    }

    #[inline]
    pub fn get_current_monitor(&self) -> RootMH {
        RootMH {
            inner: MonitorHandle
        }
    }

    #[inline]
    pub fn get_available_monitors(&self) -> VecDequeIter<MonitorHandle> {
        VecDeque::new().into_iter()
    }

    #[inline]
    pub fn get_primary_monitor(&self) -> MonitorHandle {
        MonitorHandle
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        // TODO ?
        unsafe { WindowId::dummy() }
    }
}

fn new_rootelw<T>() -> RootELW<T> {
    RootELW {
        p: EventLoopWindowTarget {
            _phantom: PhantomData
        },
        _marker: PhantomData
    }
}

pub struct EventLoop<T: 'static> {
    window_target: RootELW<T>,
    data: Rc<RefCell<EventLoopData<T>>>,
}

#[derive(Clone)]
struct EventLoopData<T> {
    events: VecDeque<Event<T>>,
    control: ControlFlow,
}

impl<T> EventLoop<T> {
    pub fn new() -> Self {
        unimplemented!();
    }

    pub fn get_available_monitors(&self) -> VecDequeIter<MonitorHandle> {
        VecDeque::new().into_iter()
    }

    pub fn get_primary_monitor(&self) -> MonitorHandle {
        MonitorHandle
    }

    pub fn run<F>(mut self, event_handler: F) -> !
        where F: 'static + FnMut(Event<T>, &RootELW<T>, &mut ControlFlow)
    {
        // TODO: Create event handlers for the JS events
        // TODO: how to handle request redraw?
        // TODO: onclose (stdweb PR)
        // TODO: file dropping, PathBuf isn't useful for web

        let document = &document();
        self.add_event(document, |data, event: BlurEvent| {
        });
        self.add_event(document, |data, event: FocusEvent| {
        });

        // TODO: what to do after attaching events
        unimplemented!();
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            data: self.data.clone()
        }
    }

    pub fn window_target(&self) -> &RootELW<T> {
        &self.window_target
    }

    // Apply all enqueued events
    fn apply_events<F>(&mut self, mut event_handler: F, start: StartCause)
        where F: 'static + FnMut(Event<T>, &RootELW<T>, &mut ControlFlow) {
        // TODO: how to handle ControlFlow::Exit?
        let mut data = self.data.borrow_mut();
        let mut control = data.control.clone();
        let events = &mut data.events;
        event_handler(Event::NewEvents(start), &new_rootelw(), &mut control);
        for event in events.drain(..) {
            event_handler(event, &new_rootelw(), &mut control);
        }
        event_handler(Event::EventsCleared, &new_rootelw(), &mut control)
    }

    fn register_window(&self, other: &Window) {
        let canvas = &other.canvas;
        
        self.add_event(canvas, |data, event: KeyDownEvent| {
            // TODO: received character
            // TODO: keyboard input
        });
        self.add_event(canvas, |data, event: KeyUpEvent| {
            // TODO: keyboard input
        });
        self.add_event(canvas, |data, _: PointerOutEvent| {
            // TODO
        });
        self.add_event(canvas, |data, _: PointerOverEvent| {
            // TODO
        });
        self.add_event(canvas, |data, event: PointerMoveEvent| {
            // TODO: mouse move
        });
        self.add_event(canvas, |data, event: PointerUpEvent| {
            // TODO: mouse pointers
        });
        self.add_event(canvas, |data, event: PointerDownEvent| {
            // TODO: mouse pointers
        });
    }

    fn add_event<E, F>(&self, target: &impl IEventTarget, mut handler: F) 
            where E: ConcreteEvent, F: FnMut(RefMut<EventLoopData<T>>, E) + 'static {
        let data = self.data.clone();

        target.add_event_listener(move |event: E| {
            event.prevent_default();
            event.stop_propagation();
            event.cancel_bubble();

            handler(data.borrow_mut(), event);
        });
    }
}

#[derive(Clone)]
pub struct EventLoopProxy<T> {
    data: Rc<RefCell<EventLoopData<T>>>
}

impl<T> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed> {
        self.data.borrow_mut().events.push_back(Event::UserEvent(event));
        Ok(())
    }
}

pub struct EventLoopWindowTarget<T> {
    _phantom: PhantomData<T>
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlatformSpecificWindowBuilderAttributes;

