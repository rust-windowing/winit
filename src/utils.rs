// A poly-fill for `lazy_cell`
// Replace with std::sync::LazyLock when https://github.com/rust-lang/rust/issues/109736 is stabilized.

// This isn't used on every platform, which can come up as dead code warnings.
#![allow(dead_code)]

use std::any::Any;
use std::ops::Deref;
use std::sync::OnceLock;

pub(crate) struct Lazy<T> {
    cell: OnceLock<T>,
    init: fn() -> T,
}

impl<T> Lazy<T> {
    pub const fn new(f: fn() -> T) -> Self {
        Self { cell: OnceLock::new(), init: f }
    }
}

impl<T> Deref for Lazy<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &'_ T {
        self.cell.get_or_init(self.init)
    }
}

pub trait AsAny {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: OpaqueObject> AsAny for T {
    #[inline(always)]
    fn as_any(&self) -> &dyn Any {
        self
    }

    #[inline(always)]
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Marker to help ensure that dynamic casting is performed on the right object.
pub trait OpaqueObject: 'static {}
