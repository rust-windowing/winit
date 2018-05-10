
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct LogicalCoordinates {
    pub x: f64,
    pub y: f64,
}

impl LogicalCoordinates {
    #[inline]
    pub fn new(x: f64, y: f64) -> Self {
        LogicalCoordinates { x, y }
    }

    #[inline]
    pub fn from_physical<T: Into<PhysicalCoordinates>>(physical: T, dpi_factor: f64) -> Self {
        physical.into().to_logical(dpi_factor)
    }

    #[inline]
    pub fn to_physical(&self, dpi_factor: f64) -> PhysicalCoordinates {
        assert!(dpi_factor > 0.0);
        let x = self.x * dpi_factor;
        let y = self.y * dpi_factor;
        PhysicalCoordinates::new(x, y)
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct PhysicalCoordinates {
    pub x: f64,
    pub y: f64,
}

impl PhysicalCoordinates {
    #[inline]
    pub fn new(x: f64, y: f64) -> Self {
        PhysicalCoordinates { x, y }
    }

    #[inline]
    pub fn from_logical(logical: LogicalCoordinates, dpi_factor: f64) -> Self {
        logical.to_physical(dpi_factor)
    }

    #[inline]
    pub fn to_logical(&self, dpi_factor: f64) -> LogicalCoordinates {
        assert!(dpi_factor > 0.0);
        let x = self.x / dpi_factor;
        let y = self.y / dpi_factor;
        LogicalCoordinates::new(x, y)
    }
}

impl From<(f64, f64)> for PhysicalCoordinates {
    #[inline]
    fn from((x, y): (f64, f64)) -> Self {
        Self::new(x, y)
    }
}

impl From<(i32, i32)> for PhysicalCoordinates {
    #[inline]
    fn from((x, y): (i32, i32)) -> Self {
        Self::new(x as f64, y as f64)
    }
}

impl Into<(i32, i32)> for PhysicalCoordinates {
    #[inline]
    fn into(self) -> (i32, i32) {
        (self.x.round() as _, self.y.round() as _)
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct LogicalDimensions {
    pub width: f64,
    pub height: f64,
}

impl LogicalDimensions {
    #[inline]
    pub fn new(width: f64, height: f64) -> Self {
        LogicalDimensions { width, height }
    }

    #[inline]
    pub fn from_physical<T: Into<PhysicalDimensions>>(physical: T, dpi_factor: f64) -> Self {
        physical.into().to_logical(dpi_factor)
    }

    #[inline]
    pub fn to_physical(&self, dpi_factor: f64) -> PhysicalDimensions {
        assert!(dpi_factor > 0.0);
        let width = self.width * dpi_factor;
        let height = self.height * dpi_factor;
        PhysicalDimensions::new(width, height)
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct PhysicalDimensions {
    pub width: f64,
    pub height: f64,
}

impl PhysicalDimensions {
    #[inline]
    pub fn new(width: f64, height: f64) -> Self {
        PhysicalDimensions { width, height }
    }

    #[inline]
    pub fn from_logical(logical: LogicalDimensions, dpi_factor: f64) -> Self {
        logical.to_physical(dpi_factor)
    }

    #[inline]
    pub fn to_logical(&self, dpi_factor: f64) -> LogicalDimensions {
        assert!(dpi_factor > 0.0);
        let width = self.width / dpi_factor;
        let height = self.height / dpi_factor;
        LogicalDimensions::new(width, height)
    }
}

impl From<(f64, f64)> for PhysicalDimensions {
    #[inline]
    fn from((width, height): (f64, f64)) -> Self {
        Self::new(width, height)
    }
}

impl From<(u32, u32)> for PhysicalDimensions {
    #[inline]
    fn from((width, height): (u32, u32)) -> Self {
        Self::new(width as f64, height as f64)
    }
}

impl Into<(u32, u32)> for PhysicalDimensions {
    #[inline]
    fn into(self) -> (u32, u32) {
        (self.width.round() as _, self.height.round() as _)
    }
}
