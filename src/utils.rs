// A poly-fill for `lazy_cell`
// Replace with std::sync::LazyLock when https://github.com/rust-lang/rust/issues/109736 is stablized.

use std::cell::Cell;
use std::ops::Deref;
use std::sync::OnceLock;

pub(crate) struct Lazy<T, F = fn() -> T> {
    cell: OnceLock<T>,
    init: Cell<Option<F>>,
}

unsafe impl<T: Send, F: Send> Sync for Lazy<T, F> {}

impl<T, F: FnOnce() -> T> Lazy<T, F> {
    pub const fn new(f: F) -> Self {
        Self {
            cell: OnceLock::new(),
            init: Cell::new(Some(f)),
        }
    }
}

impl<T, F: FnOnce() -> T> Deref for Lazy<T, F> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &'_ T {
        self.cell.get_or_init(|| match self.init.take() {
            Some(f) => f(),
            None => unreachable!(),
        })
    }
}
