use libc::dev_t;

use crate::event_loop::EventLoop;

/// Additional methods on [`EventLoop`] that are specific to platforms using the Linux
/// Direct Rendering Manager (DRM).
pub trait EventLoopExtDrm {
    /// Returns the device that the system prefers to use.
    ///
    /// The EGL EGL_EXT_device_drm and Vulkan VK_EXT_physical_device_drm extensions can be
    /// used to select a matching device for accelerated rendering.
    ///
    /// This function returns `None` if the device is unknown.
    fn main_drm_device(&self) -> Option<dev_t>;
}

impl EventLoopExtDrm for EventLoop {
    fn main_drm_device(&self) -> Option<dev_t> {
        self.event_loop.main_drm_device()
    }
}
