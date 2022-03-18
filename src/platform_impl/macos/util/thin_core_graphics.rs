#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(non_snake_case)]
#![allow(unused)]

use super::thin_core_foundation::*;
use libc;

pub mod base {
    #[cfg(any(target_arch = "x86", target_arch = "arm", target_arch = "aarch64"))]
    pub type boolean_t = libc::c_int;
    #[cfg(target_arch = "x86_64")]
    pub type boolean_t = libc::c_uint;

    #[cfg(target_pointer_width = "64")]
    pub type CGFloat = libc::c_double;
    #[cfg(not(target_pointer_width = "64"))]
    pub type CGFloat = libc::c_float;

    pub type CGError = i32;

    pub type CGGlyph = libc::c_ushort;
}

pub mod display {
    use super::{base::*, *};
    use std::ptr;

    #[derive(Copy, Clone, Debug)]
    pub struct CGDisplay {
        pub id: CGDirectDisplayID,
    }

    impl CGDisplay {
        #[inline]
        pub fn new(id: CGDirectDisplayID) -> CGDisplay {
            CGDisplay { id }
        }

        /// Returns the the main display.
        #[inline]
        pub fn main() -> CGDisplay {
            CGDisplay::new(unsafe { CGMainDisplayID() })
        }

        /// Returns the display height in pixel units.
        #[inline]
        pub fn pixels_high(&self) -> u64 {
            unsafe { CGDisplayPixelsHigh(self.id) as u64 }
        }

        /// Returns the display width in pixel units.
        #[inline]
        pub fn pixels_wide(&self) -> u64 {
            unsafe { CGDisplayPixelsWide(self.id) as u64 }
        }

        /// Returns the model number of a display monitor.
        #[inline]
        pub fn model_number(&self) -> u32 {
            unsafe { CGDisplayModelNumber(self.id) }
        }

        /// Provides a list of displays that are active (or drawable).
        #[inline]
        pub fn active_displays() -> Result<Vec<CGDirectDisplayID>, CGError> {
            let count = CGDisplay::active_display_count()?;
            let mut buf: Vec<CGDirectDisplayID> = vec![0; count as usize];
            let result =
                unsafe { CGGetActiveDisplayList(count as u32, buf.as_mut_ptr(), ptr::null_mut()) };
            if result == 0 {
                Ok(buf)
            } else {
                Err(result)
            }
        }

        /// Provides count of displays that are active (or drawable).
        #[inline]
        pub fn active_display_count() -> Result<u32, CGError> {
            let mut count: u32 = 0;
            let result = unsafe { CGGetActiveDisplayList(0, ptr::null_mut(), &mut count) };
            if result == 0 {
                Ok(count as u32)
            } else {
                Err(result)
            }
        }

        /// Moves the mouse cursor without generating events.
        #[inline]
        pub fn warp_mouse_cursor_position(point: CGPoint) -> Result<(), CGError> {
            let result = unsafe { CGWarpMouseCursorPosition(point) };
            if result == 0 {
                Ok(())
            } else {
                Err(result)
            }
        }

        /// Connects or disconnects the mouse and cursor while an application is
        /// in the foreground.
        #[inline]
        pub fn associate_mouse_and_mouse_cursor_position(connected: bool) -> Result<(), CGError> {
            let result = unsafe { CGAssociateMouseAndMouseCursorPosition(connected as boolean_t) };
            if result == 0 {
                Ok(())
            } else {
                Err(result)
            }
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug, Default)]
    pub struct CGRect {
        pub origin: CGPoint,
        pub size: CGSize,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Debug, Default)]
    pub struct CGSize {
        pub width: CGFloat,
        pub height: CGFloat,
    }

    impl CGSize {
        #[inline]
        pub fn new(width: CGFloat, height: CGFloat) -> CGSize {
            CGSize { width, height }
        }

        // #[inline]
        // pub fn apply_transform(&self, t: &CGAffineTransform) -> CGSize {
        //     unsafe { ffi::CGSizeApplyAffineTransform(*self, *t) }
        // }
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug, Default)]
    pub struct CGPoint {
        pub x: CGFloat,
        pub y: CGFloat,
    }

    impl CGPoint {
        #[inline]
        pub fn new(x: CGFloat, y: CGFloat) -> CGPoint {
            CGPoint { x, y }
        }

        // #[inline]
        // pub fn apply_transform(&self, t: &CGAffineTransform) -> CGPoint {
        //     unsafe { ffi::CGPointApplyAffineTransform(*self, *t) }
        // }
    }

    pub type CGDirectDisplayID = u32;
    pub type CGWindowID = u32;
    pub type CGDisplayConfigRef = *mut libc::c_void;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        pub fn CGMainDisplayID() -> CGDirectDisplayID;

        pub fn CGDisplayPixelsHigh(display: CGDirectDisplayID) -> libc::size_t;
        pub fn CGDisplayPixelsWide(display: CGDirectDisplayID) -> libc::size_t;
        pub fn CGDisplayModelNumber(display: CGDirectDisplayID) -> u32;
        pub fn CGDisplayBounds(display: CGDirectDisplayID) -> CGRect;
        pub fn CGDisplayModeRelease(mode: sys::CGDisplayModeRef);
        pub fn CGGetActiveDisplayList(
            max_displays: u32,
            active_displays: *mut CGDirectDisplayID,
            display_count: *mut u32,
        ) -> CGError;
        pub fn CGWarpMouseCursorPosition(point: CGPoint) -> CGError;
        pub fn CGAssociateMouseAndMouseCursorPosition(connected: boolean_t) -> CGError;
    }

    mod sys {
        #[cfg(target_os = "macos")]
        mod macos {
            pub enum CGEventTap {}
            pub type CGEventTapRef = crate::platform_impl::thin_core_foundation::CFMachPortRef;
            pub enum CGEvent {}
            pub type CGEventRef = *mut CGEvent;

            pub enum CGEventSource {}
            pub type CGEventSourceRef = *mut CGEventSource;

            pub enum CGDisplayMode {}
            pub type CGDisplayModeRef = *mut CGDisplayMode;
        }

        #[cfg(target_os = "macos")]
        pub use self::macos::*;
    }

    use super::foreign_types::*;

    crate::foreign_type! {
        #[doc(hidden)]
        type CType = sys::CGDisplayMode;
        fn drop = CGDisplayModeRelease;
        fn clone = |p| CFRetain(p as *const _) as *mut _;
        pub struct CGDisplayMode;
        pub struct CGDisplayModeRef;
    }

    #[macro_export]
    macro_rules! foreign_type {
    (
        $(#[$impl_attr:meta])*
        type CType = $ctype:ty;
        fn drop = $drop:expr;
        $(fn clone = $clone:expr;)*
        $(#[$owned_attr:meta])*
        pub struct $owned:ident;
        $(#[$borrowed_attr:meta])*
        pub struct $borrowed:ident;
    ) => {
        $(#[$owned_attr])*
        pub struct $owned(*mut $ctype);

        $(#[$impl_attr])*
        impl ForeignType for $owned {
            type CType = $ctype;
            type Ref = $borrowed;

            #[inline]
            unsafe fn from_ptr(ptr: *mut $ctype) -> $owned {
                $owned(ptr)
            }

            #[inline]
            fn as_ptr(&self) -> *mut $ctype {
                self.0
            }
        }

        impl Drop for $owned {
            #[inline]
            fn drop(&mut self) {
                unsafe { $drop(self.0) }
            }
        }

        $(
            impl Clone for $owned {
                #[inline]
                fn clone(&self) -> $owned {
                    unsafe {
                        let handle: *mut $ctype = $clone(self.0);
                        ForeignType::from_ptr(handle)
                    }
                }
            }

            impl ::std::borrow::ToOwned for $borrowed {
                type Owned = $owned;
                #[inline]
                fn to_owned(&self) -> $owned {
                    unsafe {
                        let handle: *mut $ctype = $clone(ForeignTypeRef::as_ptr(self));
                        ForeignType::from_ptr(handle)
                    }
                }
            }
        )*

        impl ::std::ops::Deref for $owned {
            type Target = $borrowed;

            #[inline]
            fn deref(&self) -> &$borrowed {
                unsafe { ForeignTypeRef::from_ptr(self.0) }
            }
        }

        impl ::std::ops::DerefMut for $owned {
            #[inline]
            fn deref_mut(&mut self) -> &mut $borrowed {
                unsafe { ForeignTypeRef::from_ptr_mut(self.0) }
            }
        }

        impl ::std::borrow::Borrow<$borrowed> for $owned {
            #[inline]
            fn borrow(&self) -> &$borrowed {
                &**self
            }
        }

        impl ::std::convert::AsRef<$borrowed> for $owned {
            #[inline]
            fn as_ref(&self) -> &$borrowed {
                &**self
            }
        }

        $(#[$borrowed_attr])*
        pub struct $borrowed(Opaque);

        $(#[$impl_attr])*
        impl ForeignTypeRef for $borrowed {
            type CType = $ctype;
        }
    }
}
}

mod foreign_types {
    use core::cell::UnsafeCell;

    /// An opaque type used to define `ForeignTypeRef` types.
    ///
    /// A type implementing `ForeignTypeRef` should simply be a newtype wrapper around this type.
    pub struct Opaque(UnsafeCell<()>);

    /// A type implemented by wrappers over foreign types.
    pub trait ForeignType: Sized {
        /// The raw C type.
        type CType;

        /// The type representing a reference to this type.
        type Ref: ForeignTypeRef<CType = Self::CType>;

        /// Constructs an instance of this type from its raw type.
        unsafe fn from_ptr(ptr: *mut Self::CType) -> Self;

        /// Returns a raw pointer to the wrapped value.
        fn as_ptr(&self) -> *mut Self::CType;
    }

    /// A trait implemented by types which reference borrowed foreign types.
    pub trait ForeignTypeRef: Sized {
        /// The raw C type.
        type CType;

        /// Constructs a shared instance of this type from its raw type.
        #[inline]
        unsafe fn from_ptr<'a>(ptr: *mut Self::CType) -> &'a Self {
            &*(ptr as *mut _)
        }

        /// Constructs a mutable reference of this type from its raw type.
        #[inline]
        unsafe fn from_ptr_mut<'a>(ptr: *mut Self::CType) -> &'a mut Self {
            &mut *(ptr as *mut _)
        }

        /// Returns a raw pointer to the wrapped value.
        #[inline]
        fn as_ptr(&self) -> *mut Self::CType {
            self as *const _ as *mut _
        }
    }
}
