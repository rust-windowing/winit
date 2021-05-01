use crate::{
    dpi::{PhysicalSize, PhysicalPosition},
    monitor::{ MonitorHandle as RootMonitorHandle, VideoMode as RootVideoMode},
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MonitorHandle;

impl MonitorHandle {
    #[inline]
    pub fn name(&self) -> Option<String> {
        todo!()
    }

    #[inline]
    pub fn size(&self) -> PhysicalSize<u32> {
        todo!()
    }

    #[inline]
    pub fn position(&self) -> PhysicalPosition<i32> {
        todo!()
    }

    #[inline]
    pub fn scale_factor(&self) -> f64 {
        todo!()
    }

    #[inline]
    pub fn video_modes(&self) -> Box<dyn Iterator<Item = RootVideoMode>> {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VideoMode;

impl VideoMode {
    #[inline]
    pub fn size(&self) -> PhysicalSize<u32> {
        todo!()
    }

    #[inline]
    pub fn bit_depth(&self) -> u16 {
        todo!()
    }

    #[inline]
    pub fn refresh_rate(&self) -> u16 {
        todo!()
    }

    #[inline]
    pub fn monitor(&self) -> RootMonitorHandle {
        todo!()
    }
}

