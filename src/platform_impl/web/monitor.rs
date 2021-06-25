use crate::dpi::{PhysicalPosition, PhysicalSize};
use crate::monitor::{MonitorHandle, VideoMode};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Handle;

impl Handle {
    pub fn scale_factor(&self) -> f64 {
        1.0
    }

    pub fn position(&self) -> PhysicalPosition<i32> {
        PhysicalPosition { x: 0, y: 0 }
    }

    pub fn name(&self) -> Option<String> {
        None
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        PhysicalSize {
            width: 0,
            height: 0,
        }
    }

    pub fn video_modes(&self) -> impl Iterator<Item = VideoMode> {
        std::iter::empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Mode;

impl Mode {
    pub fn size(&self) -> PhysicalSize<u32> {
        unimplemented!();
    }

    pub fn bit_depth(&self) -> u16 {
        unimplemented!();
    }

    pub fn refresh_rate(&self) -> u16 {
        32
    }

    pub fn monitor(&self) -> MonitorHandle {
        MonitorHandle { inner: Handle }
    }
}
