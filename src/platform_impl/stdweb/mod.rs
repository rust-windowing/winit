use dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use event::Event;
use event_loop::{ControlFlow, EventLoopWindowTarget as RootELW, EventLoopClosed};
use icon::Icon;
use monitor::{MonitorHandle as RootMH};
use window::{CreationError, MouseCursor, WindowAttributes};

use std::collections::vec_deque::IntoIter as VecDequeIter;
use std::marker::PhantomData;

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

pub struct Window;

impl Window {
    // TODO: type of window_target
    pub fn new<T>(target: &EventLoopWindowTarget<T>, window: WindowAttributes, platform: PlatformSpecificWindowBuilderAttributes) -> Result<Self, CreationError> {
        unimplemented!();
    }

    pub fn set_title(&self, title: &str) {
        unimplemented!();
    }

    pub fn show(&self) {
        unimplemented!();
    }

    pub fn hide(&self) {
        unimplemented!();
    }

    pub fn request_redraw(&self) {
        unimplemented!();
    }

    pub fn get_position(&self) -> Option<LogicalPosition> {
        unimplemented!();
    }

    pub fn get_inner_position(&self) -> Option<LogicalPosition> {
        unimplemented!();
    }

    pub fn set_position(&self, position: LogicalPosition) {
        unimplemented!();
    }

    #[inline]
    pub fn get_inner_size(&self) -> Option<LogicalSize> {
        unimplemented!();
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<LogicalSize> {
        unimplemented!();
    }

    #[inline]
    pub fn set_inner_size(&self, size: LogicalSize) {
        unimplemented!();
    }

    #[inline]
    pub fn set_min_dimensions(&self, dimensions: Option<LogicalSize>) {
        unimplemented!();
    }

    #[inline]
    pub fn set_max_dimensions(&self, dimensions: Option<LogicalSize>) {
        unimplemented!();
    }

    #[inline]
    pub fn set_resizable(&self, resizable: bool) {
        unimplemented!();
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f64 {
        unimplemented!();
    }

    #[inline]
    pub fn set_cursor(&self, cursor: MouseCursor) {
        unimplemented!();
    }

    #[inline]
    pub fn set_cursor_position(&self, position: LogicalPosition) -> Result<(), String> {
        unimplemented!();
    }

    #[inline]
    pub fn grab_cursor(&self, grab: bool) -> Result<(), String> {
        unimplemented!();
    }

    #[inline]
    pub fn hide_cursor(&self, hide: bool) {
        unimplemented!();
    }

    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        unimplemented!();
    }

    #[inline]
    pub fn set_fullscreen(&self, monitor: Option<RootMH>) {
        unimplemented!();
    }

    #[inline]
    pub fn set_decorations(&self, decorations: bool) {
        unimplemented!();
    }

    #[inline]
    pub fn set_always_on_top(&self, always_on_top: bool) {
        unimplemented!();
    }

    #[inline]
    pub fn set_window_icon(&self, window_icon: Option<Icon>) {
        unimplemented!();
    }

    #[inline]
    pub fn set_ime_spot(&self, position: LogicalPosition) {
        unimplemented!();
    }

    #[inline]
    pub fn get_current_monitor(&self) -> RootMH {
        unimplemented!();
    }

    #[inline]
    pub fn get_available_monitors(&self) -> VecDequeIter<MonitorHandle> {
        unimplemented!();
    }

    #[inline]
    pub fn get_primary_monitor(&self) -> MonitorHandle {
        unimplemented!();
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        unimplemented!();
    }
}

pub struct EventLoop<T> {
    _phantom: PhantomData<T>
}

impl<T> EventLoop<T> {
    pub fn new() -> Self {
        unimplemented!();
    }

    pub fn get_available_monitors(&self) -> VecDequeIter<MonitorHandle> {
        unimplemented!();
    }

    pub fn get_primary_monitor(&self) -> MonitorHandle {
        unimplemented!();
    }

    pub fn run<F>(mut self, event_handler: F) -> !
        where F: 'static + FnMut(Event<T>, &RootELW<T>, &mut ControlFlow)
    {
        unimplemented!();
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        unimplemented!();
    }

    pub fn window_target(&self) -> &RootELW<T> {
        unimplemented!();
        /*&EventLoopWindowTarget {
            p: self.event_loop.window_target(),
            _marker: std::marker::PhantomData
        }*/
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventLoopProxy<T> {
    _phantom: PhantomData<T>
}

impl<T> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed> {
        unimplemented!();
    }
}

pub struct EventLoopWindowTarget<T> {
    _phantom: PhantomData<T>
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlatformSpecificWindowBuilderAttributes;

