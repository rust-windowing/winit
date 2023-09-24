use std::iter::Empty;

use crate::dpi::{PhysicalPosition, PhysicalSize};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MonitorHandle;

impl MonitorHandle {
    pub fn scale_factor(&self) -> f64 {
        unreachable!()
    }

    pub fn position(&self) -> PhysicalPosition<i32> {
        unreachable!()
    }

    pub fn name(&self) -> Option<String> {
        unreachable!()
    }

    pub fn refresh_rate_millihertz(&self) -> Option<u32> {
        unreachable!()
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        unreachable!()
    }

    pub fn video_modes(&self) -> Empty<VideoMode> {
        unreachable!()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VideoMode;

impl VideoMode {
    pub fn size(&self) -> PhysicalSize<u32> {
        unreachable!();
    }

    pub fn bit_depth(&self) -> u16 {
        unreachable!();
    }

    pub fn refresh_rate_millihertz(&self) -> u32 {
        unreachable!();
    }

    pub fn monitor(&self) -> MonitorHandle {
        unreachable!();
    }
}
