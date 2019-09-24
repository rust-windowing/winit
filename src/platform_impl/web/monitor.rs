use crate::dpi::{PhysicalPosition, PhysicalSize};
use crate::monitor::VideoMode;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Handle;

impl Handle {
    pub fn hidpi_factor(&self) -> f64 {
        1.0
    }

    pub fn position(&self) -> PhysicalPosition {
        PhysicalPosition {
            x: 0.0,
            y: 0.0,
        }
    }

    pub fn name(&self) -> Option<String> {
        None
    }

    pub fn size(&self) -> PhysicalSize {
        PhysicalSize {
            width: 0.0,
            height: 0.0
        }
    }

    pub fn video_modes(&self) -> impl Iterator<Item = VideoMode> {
        std::iter::empty()
    }
}
