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

// NOTE: This is `pub`, but isn't actually exposed outside the crate.
// NOTE: Marked as `#[doc(hidden)]` and underscored, because they can be quite difficult to use
// correctly, see discussion in #4160.
// FIXME: Remove and replace with a coercion once rust-lang/rust#65991 is in MSRV (1.86).
#[doc(hidden)]
pub trait AsAny: Any {
    #[doc(hidden)]
    fn __as_any(&self) -> &dyn Any;
    #[doc(hidden)]
    fn __as_any_mut(&mut self) -> &mut dyn Any;
    #[doc(hidden)]
    fn __into_any(self: Box<Self>) -> Box<dyn Any>;
}

impl<T: Any> AsAny for T {
    #[inline(always)]
    fn __as_any(&self) -> &dyn Any {
        self
    }

    #[inline(always)]
    fn __as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    #[inline(always)]
    fn __into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}
