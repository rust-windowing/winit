use crate::dpi::{PhysicalPosition, PhysicalSize};
use crate::monitor::VideoMode;

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
