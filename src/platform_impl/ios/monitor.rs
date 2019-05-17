use std::collections::VecDeque;
use std::fmt;

use dpi::{PhysicalPosition, PhysicalSize};

use platform_impl::platform::ffi::{
    id,
    nil,
    CGFloat,
    CGRect,
    NSUInteger,
};

pub struct MonitorHandle {
    uiscreen: id,
}

unsafe impl Send for MonitorHandle {}
unsafe impl Sync for MonitorHandle {}

impl Clone for MonitorHandle {
    fn clone(&self) -> MonitorHandle {
        MonitorHandle::retained_new(self.uiscreen)
    }
}

impl Drop for MonitorHandle {
    fn drop(&mut self) {
        unsafe {
            let () = msg_send![self.uiscreen, release];
        }
    }
}

impl fmt::Debug for MonitorHandle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        #[derive(Debug)]
        struct MonitorHandle {
            name: Option<String>,
            dimensions: PhysicalSize,
            position: PhysicalPosition,
            hidpi_factor: f64,
        }

        let monitor_id_proxy = MonitorHandle {
            name: self.get_name(),
            dimensions: self.get_dimensions(),
            position: self.get_position(),
            hidpi_factor: self.get_hidpi_factor(),
        };

        monitor_id_proxy.fmt(f)
    }
}

impl MonitorHandle {
    pub fn retained_new(uiscreen: id) -> MonitorHandle {
        unsafe {
            let () = msg_send![uiscreen, retain];
        }
        MonitorHandle { uiscreen }
    }

    pub fn get_name(&self) -> Option<String> {
        unsafe {
            assert_main_thread!("`MonitorHandle::get_name` can only be `called` on the main thread on iOS");
            if self.uiscreen == main_uiscreen().uiscreen {
                Some("Primary".to_string())
            } else if self.uiscreen == mirrored_uiscreen().uiscreen {
                Some("Mirrored".to_string())
            } else {
                uiscreens()
                    .iter()
                    .position(|rhs| rhs.uiscreen == self.uiscreen)
                    .map(|idx| idx.to_string())
            }
        }
    }
    
    pub fn get_dimensions(&self) -> PhysicalSize {
        unsafe {
            assert_main_thread!("`MonitorHandle::get_dimensions` can only be `called` on the main thread on iOS");
            let bounds: CGRect = msg_send![self.get_uiscreen(), nativeBounds];
            (bounds.size.width as f64, bounds.size.height as f64).into()
        }
    }
    
    pub fn get_position(&self) -> PhysicalPosition {
        unsafe {
            assert_main_thread!("`MonitorHandle::get_position` can only be `called` on the main thread on iOS");
            let bounds: CGRect = msg_send![self.get_uiscreen(), nativeBounds];
            (bounds.origin.x as f64, bounds.origin.y as f64).into()
        }
    }
    
    pub fn get_hidpi_factor(&self) -> f64 {
        unsafe {
            assert_main_thread!("`MonitorHandle::get_hidpi_factor` can only be `called` on the main thread on iOS");
            let scale: CGFloat = msg_send![self.get_uiscreen(), nativeScale];
            scale as f64
        }
    }
}

// MonitorHandleExtIOS
impl MonitorHandle {
    pub fn get_uiscreen(&self) -> id {
        self.uiscreen
    }
}

// requires being run on main thread
pub unsafe fn main_uiscreen() -> MonitorHandle {
    let uiscreen: id = msg_send![class!(UIScreen), mainScreen];
    MonitorHandle::retained_new(uiscreen)
}

// requires being run on main thread
unsafe fn mirrored_uiscreen() -> MonitorHandle {
    let uiscreen: id = msg_send![class!(UIScreen), mirroredScreen];
    MonitorHandle::retained_new(uiscreen)
}

// requires being run on main thread
pub unsafe fn uiscreens() -> VecDeque<MonitorHandle> {
    let screens: id = msg_send![class!(UIScreen), screens];
    let count: NSUInteger = msg_send![screens, count];
    let mut result = VecDeque::with_capacity(count as _);
    let screens_enum: id = msg_send![screens, objectEnumerator];
    loop {
        let screen: id = msg_send![screens_enum, nextObject];
        if screen == nil {
            break result
        }
        result.push_back(MonitorHandle::retained_new(screen));
    }
}