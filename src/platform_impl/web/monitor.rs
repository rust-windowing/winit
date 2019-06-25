use crate::dpi::{PhysicalPosition, PhysicalSize};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Handle;

impl Handle {
    pub fn hidpi_factor(&self) -> f64 {
        1.0
    }

    pub fn position(&self) -> PhysicalPosition {
        unimplemented!();
    }

    pub fn dimensions(&self) -> PhysicalSize {
        unimplemented!();
    }

    pub fn name(&self) -> Option<String> {
        unimplemented!();
    }
}
