
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct LogicalPosition {
    pub x: f64,
    pub y: f64,
}

impl LogicalPosition {
    #[inline]
    pub fn new(x: f64, y: f64) -> Self {
        LogicalPosition { x, y }
    }

    #[inline]
    pub fn from_physical<T: Into<PhysicalPosition>>(physical: T, dpi_factor: f64) -> Self {
        physical.into().to_logical(dpi_factor)
    }

    #[inline]
    pub fn to_physical(&self, dpi_factor: f64) -> PhysicalPosition {
        assert!(dpi_factor > 0.0);
        let x = self.x * dpi_factor;
        let y = self.y * dpi_factor;
        PhysicalPosition::new(x, y)
    }
}

impl From<(f64, f64)> for LogicalPosition {
    #[inline]
    fn from((x, y): (f64, f64)) -> Self {
        Self::new(x, y)
    }
}

impl From<(i32, i32)> for LogicalPosition {
    #[inline]
    fn from((x, y): (i32, i32)) -> Self {
        Self::new(x as f64, y as f64)
    }
}

impl Into<(f64, f64)> for LogicalPosition {
    #[inline]
    fn into(self) -> (f64, f64) {
        (self.x, self.y)
    }
}

impl Into<(i32, i32)> for LogicalPosition {
    #[inline]
    fn into(self) -> (i32, i32) {
        (self.x.round() as _, self.y.round() as _)
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct PhysicalPosition {
    pub x: f64,
    pub y: f64,
}

impl PhysicalPosition {
    #[inline]
    pub fn new(x: f64, y: f64) -> Self {
        PhysicalPosition { x, y }
    }

    #[inline]
    pub fn from_logical<T: Into<LogicalPosition>>(logical: T, dpi_factor: f64) -> Self {
        logical.into().to_physical(dpi_factor)
    }

    #[inline]
    pub fn to_logical(&self, dpi_factor: f64) -> LogicalPosition {
        assert!(dpi_factor > 0.0);
        let x = self.x / dpi_factor;
        let y = self.y / dpi_factor;
        LogicalPosition::new(x, y)
    }
}

impl From<(f64, f64)> for PhysicalPosition {
    #[inline]
    fn from((x, y): (f64, f64)) -> Self {
        Self::new(x, y)
    }
}

impl From<(i32, i32)> for PhysicalPosition {
    #[inline]
    fn from((x, y): (i32, i32)) -> Self {
        Self::new(x as f64, y as f64)
    }
}

impl Into<(f64, f64)> for PhysicalPosition {
    #[inline]
    fn into(self) -> (f64, f64) {
        (self.x, self.y)
    }
}

impl Into<(i32, i32)> for PhysicalPosition {
    #[inline]
    fn into(self) -> (i32, i32) {
        (self.x.round() as _, self.y.round() as _)
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct LogicalSize {
    pub width: f64,
    pub height: f64,
}

impl LogicalSize {
    #[inline]
    pub fn new(width: f64, height: f64) -> Self {
        LogicalSize { width, height }
    }

    #[inline]
    pub fn from_physical<T: Into<PhysicalSize>>(physical: T, dpi_factor: f64) -> Self {
        physical.into().to_logical(dpi_factor)
    }

    #[inline]
    pub fn to_physical(&self, dpi_factor: f64) -> PhysicalSize {
        assert!(dpi_factor > 0.0);
        let width = self.width * dpi_factor;
        let height = self.height * dpi_factor;
        PhysicalSize::new(width, height)
    }
}

impl From<(f64, f64)> for LogicalSize {
    #[inline]
    fn from((width, height): (f64, f64)) -> Self {
        Self::new(width, height)
    }
}

impl From<(u32, u32)> for LogicalSize {
    #[inline]
    fn from((width, height): (u32, u32)) -> Self {
        Self::new(width as f64, height as f64)
    }
}

impl Into<(f64, f64)> for LogicalSize {
    #[inline]
    fn into(self) -> (f64, f64) {
        (self.width, self.height)
    }
}

impl Into<(u32, u32)> for LogicalSize {
    #[inline]
    fn into(self) -> (u32, u32) {
        (self.width.round() as _, self.height.round() as _)
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct PhysicalSize {
    pub width: f64,
    pub height: f64,
}

impl PhysicalSize {
    #[inline]
    pub fn new(width: f64, height: f64) -> Self {
        PhysicalSize { width, height }
    }

    #[inline]
    pub fn from_logical<T: Into<LogicalSize>>(logical: T, dpi_factor: f64) -> Self {
        logical.into().to_physical(dpi_factor)
    }

    #[inline]
    pub fn to_logical(&self, dpi_factor: f64) -> LogicalSize {
        assert!(dpi_factor > 0.0);
        let width = self.width / dpi_factor;
        let height = self.height / dpi_factor;
        LogicalSize::new(width, height)
    }
}

impl From<(f64, f64)> for PhysicalSize {
    #[inline]
    fn from((width, height): (f64, f64)) -> Self {
        Self::new(width, height)
    }
}

impl From<(u32, u32)> for PhysicalSize {
    #[inline]
    fn from((width, height): (u32, u32)) -> Self {
        Self::new(width as f64, height as f64)
    }
}

impl Into<(f64, f64)> for PhysicalSize {
    #[inline]
    fn into(self) -> (f64, f64) {
        (self.width, self.height)
    }
}

impl Into<(u32, u32)> for PhysicalSize {
    #[inline]
    fn into(self) -> (u32, u32) {
        (self.width.round() as _, self.height.round() as _)
    }
}
