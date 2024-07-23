// A poly-fill for `lazy_cell`
// Replace with std::sync::LazyLock when https://github.com/rust-lang/rust/issues/109736 is stabilized.

// This isn't used on every platform, which can come up as dead code warnings.
#![allow(dead_code)]

use std::cell::OnceCell;
use std::ops::Deref;
use std::sync::OnceLock;

pub(crate) struct LazyLock<T> {
    cell: OnceLock<T>,
    init: fn() -> T,
}

impl<T> LazyLock<T> {
    pub const fn new(f: fn() -> T) -> Self {
        Self { cell: OnceLock::new(), init: f }
    }
}

impl<T> Deref for LazyLock<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &'_ T {
        self.cell.get_or_init(self.init)
    }
}

pub(crate) struct LazyCell<T, F = fn() -> T> {
    cell: OnceCell<T>,
    init: F,
}
impl<T, F: Fn() -> T> LazyCell<T, F> {
    pub const fn new(f: F) -> Self {
        Self { cell: OnceCell::new(), init: f }
    }
}

impl<T, F: Fn() -> T> Deref for LazyCell<T, F> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &'_ T {
        self.cell.get_or_init(&self.init)
    }
}
