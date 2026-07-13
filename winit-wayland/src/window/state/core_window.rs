use std::sync::Arc;

use dpi::{PhysicalSize, Size};
use sctk::compositor::SurfaceData;
use sctk::shell::WaylandSurface;
use wayland_client::Proxy;
use winit_core::cursor::Cursor;
use winit_core::monitor::MonitorHandle as CoreMonitorHandle;

use super::super::output::MonitorHandle;
use super::WindowState;

impl WindowState {
    pub fn current_monitor(&self) -> Option<CoreMonitorHandle> {
        let data = self.window.wl_surface().data::<SurfaceData>()?;
        data.outputs()
            .next()
            .map(MonitorHandle::new)
            .map(|monitor| CoreMonitorHandle(Arc::new(monitor)))
    }

    pub fn set_cursor(&mut self, cursor: Cursor) {
        match cursor {
            Cursor::Icon(icon) => self.set_cursor_icon(icon),
            Cursor::Custom(cursor) => self.set_custom_cursor(cursor),
        }
    }

    pub fn set_surface_resize_increments(&mut self, increments: Option<Size>) {
        let increments = increments.map(|size| size.to_logical(self.scale_factor()));
        self.set_resize_increments(increments);
    }

    pub fn surface_resize_increments(&self) -> Option<PhysicalSize<u32>> {
        self.resize_increments()
            .map(|size| super::logical_to_physical_rounded(size, self.scale_factor()))
    }
}
