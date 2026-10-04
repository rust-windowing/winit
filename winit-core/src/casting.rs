#[macro_export]
macro_rules! impl_dyn_casting {
    ($trait:ident) => {
        impl dyn $trait + '_ {
            /// Downcast to the backend concrete type.
            ///
            /// Returns `None` if the object was not from that backend.
            pub fn cast_ref<T: $trait>(&self) -> Option<&T> {
                let this: &dyn std::any::Any = self;
                this.downcast_ref::<T>()
            }

            /// Mutable downcast to the backend concrete type.
            ///
            /// Returns `None` if the object was not from that backend.
            pub fn cast_mut<T: $trait>(&mut self) -> Option<&mut T> {
                let this: &mut dyn std::any::Any = self;
                this.downcast_mut::<T>()
            }

            /// Owned downcast to the backend concrete type.
            ///
            /// Returns `Err` with `self` if the object was not from that backend.
            pub fn cast<T: $trait>(self: Box<Self>) -> Result<Box<T>, Box<Self>> {
                if self.cast_ref::<T>().is_some() {
                    let this: Box<dyn std::any::Any> = self;
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
    use std::any::Any;

    struct Foo;
    trait FooTrait: Any {}
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
