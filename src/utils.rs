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

macro_rules! impl_dyn_casting {
    ($trait:ident) => {
        impl dyn $trait + '_ {
            /// Downcast to the backend concrete type.
            ///
            /// Returns `None` if the object was not from that backend.
            pub fn cast_ref<T: $trait>(&self) -> Option<&T> {
                let this: &dyn std::any::Any = self.__as_any();
                this.downcast_ref::<T>()
            }

            /// Mutable downcast to the backend concrete type.
            ///
            /// Returns `None` if the window was not from that backend.
            pub fn cast_mut<T: $trait>(&mut self) -> Option<&mut T> {
                let this: &mut dyn std::any::Any = self.__as_any_mut();
                this.downcast_mut::<T>()
            }

            /// Owned downcast to the backend concrete type.
            ///
            /// Returns `Err` with `self` if the concrete was not from that backend.
            pub fn cast<T: $trait>(self: Box<Self>) -> Result<Box<T>, Box<Self>> {
                let reference: &dyn std::any::Any = self.__as_any();
                if reference.is::<T>() {
                    let this: Box<dyn std::any::Any> = self.__into_any();
                    // Unwrap is okay, we just checked the type of `self` is `T`.
                    Ok(this.downcast::<T>().unwrap())
                } else {
                    Err(self)
                }
            }
        }
    };
}

pub(crate) use impl_dyn_casting;
