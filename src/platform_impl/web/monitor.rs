use crate::dpi::{PhysicalPosition, PhysicalSize};
use crate::monitor::{MonitorHandle, VideoMode};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Handle;

impl Handle {
    pub fn hidpi_factor(&self) -> f64 {
        1.0
    }

    pub fn position(&self) -> PhysicalPosition {
        unimplemented!();
    }

    pub fn name(&self) -> Option<String> {
        unimplemented!();
    }

    pub fn size(&self) -> PhysicalSize {
        unimplemented!();
    }

    pub fn video_modes(&self) -> impl Iterator<Item = VideoMode> {
        // TODO: is this possible ?
        std::iter::empty()
    }
}

#[derive(Derivative)]
#[derivative(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Mode;

impl Mode {
    pub fn size(&self) -> PhysicalSize {
        unimplemented!();
    }

    pub fn bit_depth(&self) -> u16 {
        unimplemented!();
    }

    pub fn refresh_rate(&self) -> u16 {
        32
    }

    pub fn monitor(&self) -> MonitorHandle {
        MonitorHandle {
            inner: Handle
        }
    }
}

