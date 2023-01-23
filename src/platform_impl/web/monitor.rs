use crate::dpi::{PhysicalPosition, PhysicalSize};
use crate::monitor::MonitorGone;

// This is never constructed, so it is fine to leave as `unimplemented!()`.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MonitorHandle(());

impl MonitorHandle {
    pub fn scale_factor(&self) -> Result<f64, MonitorGone> {
        unimplemented!()
    }

    pub fn position(&self) -> Result<PhysicalPosition<i32>, MonitorGone> {
        unimplemented!()
    }

    pub fn name(&self) -> Result<String, MonitorGone> {
        unimplemented!()
    }

    pub fn refresh_rate_millihertz(&self) -> Result<u32, MonitorGone> {
        unimplemented!()
    }

    pub fn size(&self) -> Result<PhysicalSize<u32>, MonitorGone> {
        unimplemented!()
    }

    pub fn video_modes(&self) -> Result<impl Iterator<Item = VideoMode>, MonitorGone> {
        Ok(std::iter::empty())
    }
}

// This is never constructed, so it is fine to leave as `unimplemented!()`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VideoMode(());

impl VideoMode {
    pub fn size(&self) -> PhysicalSize<u32> {
        unimplemented!()
    }

    pub fn bit_depth(&self) -> u16 {
        unimplemented!()
    }

    pub fn refresh_rate_millihertz(&self) -> u32 {
        unimplemented!()
    }

    pub fn monitor(&self) -> MonitorHandle {
        unimplemented!()
    }
}
