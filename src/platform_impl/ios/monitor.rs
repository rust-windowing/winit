use std::{
    collections::{HashSet, VecDeque},
    fmt,
    ops::{Deref, DerefMut},
};

use crate::dpi::{PhysicalPosition, PhysicalSize};
use crate::monitor::VideoMode;

use crate::platform_impl::platform::ffi::{id, nil, CGFloat, CGRect, CGSize, NSInteger, NSUInteger};

pub struct Inner {
    uiscreen: id,
}

impl Drop for Inner {
    fn drop(&mut self) {
        unsafe {
            let () = msg_send![self.uiscreen, release];
        }
    }
}

pub struct MonitorHandle {
    inner: Inner,
}

impl Deref for MonitorHandle {
    type Target = Inner;

    fn deref(&self) -> &Inner {
        unsafe {
            assert_main_thread!("`MonitorHandle` methods can only be run on the main thread on iOS");
        }
        &self.inner
    }
}

impl DerefMut for MonitorHandle {
    fn deref_mut(&mut self) -> &mut Inner {
        unsafe {
            assert_main_thread!("`MonitorHandle` methods can only be run on the main thread on iOS");
        }
        &mut self.inner
    }
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
            assert_main_thread!("`MonitorHandle` can only be dropped on the main thread on iOS");
        }
    }
}

impl fmt::Debug for MonitorHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[derive(Debug)]
        struct MonitorHandle {
            name: Option<String>,
            size: PhysicalSize,
            position: PhysicalPosition,
            hidpi_factor: f64,
        }

        let monitor_id_proxy = MonitorHandle {
            name: self.name(),
            size: self.size(),
            position: self.position(),
            hidpi_factor: self.hidpi_factor(),
        };

        monitor_id_proxy.fmt(f)
    }
}

impl MonitorHandle {
    pub fn retained_new(uiscreen: id) -> MonitorHandle {
        unsafe {
            assert_main_thread!("`MonitorHandle` can only be cloned on the main thread on iOS");
            let () = msg_send![uiscreen, retain];
        }
        MonitorHandle { inner: Inner { uiscreen } }
    }
}

impl Inner {
    pub fn name(&self) -> Option<String> {
        unsafe {
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

    pub fn size(&self) -> PhysicalSize {
        unsafe {
            let bounds: CGRect = msg_send![self.ui_screen(), nativeBounds];
            (bounds.size.width as f64, bounds.size.height as f64).into()
        }
    }

    pub fn position(&self) -> PhysicalPosition {
        unsafe {
            let bounds: CGRect = msg_send![self.ui_screen(), nativeBounds];
            (bounds.origin.x as f64, bounds.origin.y as f64).into()
        }
    }

    pub fn hidpi_factor(&self) -> f64 {
        unsafe {
            let scale: CGFloat = msg_send![self.ui_screen(), nativeScale];
            scale as f64
        }
    }

    pub fn video_modes(&self) -> impl Iterator<Item = VideoMode> {
        let refresh_rate: NSInteger = unsafe { msg_send![self.uiscreen, maximumFramesPerSecond] };

        let available_modes: id = unsafe { msg_send![self.uiscreen, availableModes] };
        let available_mode_count: NSUInteger = unsafe { msg_send![available_modes, count] };

        let mut modes = HashSet::with_capacity(available_mode_count);

        for i in 0..available_mode_count {
            let mode: id = unsafe { msg_send![available_modes, objectAtIndex: i] };
            let size: CGSize = unsafe { msg_send![mode, size] };
            modes.insert(VideoMode {
                size: (size.width as u32, size.height as u32),
                bit_depth: 32,
                refresh_rate: refresh_rate as u16,
            });
        }

        modes.into_iter()
    }
}

// MonitorHandleExtIOS
impl Inner {
    pub fn ui_screen(&self) -> id {
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
