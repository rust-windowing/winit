use std::any::Any;

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

#[macro_export]
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
            /// Returns `None` if the object was not from that backend.
            pub fn cast_mut<T: $trait>(&mut self) -> Option<&mut T> {
                let this: &mut dyn std::any::Any = self.__as_any_mut();
                this.downcast_mut::<T>()
            }

            /// Owned downcast to the backend concrete type.
            ///
            /// Returns `Err` with `self` if the object was not from that backend.
            pub fn cast<T: $trait>(self: Box<Self>) -> Result<Box<T>, Box<Self>> {
                if self.cast_ref::<T>().is_some() {
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

pub use impl_dyn_casting;

#[cfg(test)]
mod tests {
    use super::AsAny;

    struct Foo;
    trait FooTrait: AsAny {}
    impl FooTrait for Foo {}
    impl_dyn_casting!(FooTrait);

    #[test]
    fn dyn_casting() {
        let foo_owned: Box<dyn FooTrait> = Box::new(Foo);
        assert!(foo_owned.cast::<Foo>().is_ok());

        let mut foo = Foo;
        let foo_ref: &mut dyn FooTrait = &mut foo;
        assert!((foo_ref).cast_ref::<Foo>().is_some());
        assert!((&&&&foo_ref).cast_ref::<Foo>().is_some());
        assert!(foo_ref.cast_mut::<Foo>().is_some());
    }
}
