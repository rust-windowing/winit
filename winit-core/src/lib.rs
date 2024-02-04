//! Base types for a windowing library.
//!
//! This crate contains types, traits and basic functions from [`winit`] that are platform
//! independent. It is intended to allow for other crates to build abstractions around [`winit`]
//! without needing to pull in all of [`winit`]'s dependencies, as well as to provide an
//! interface for alternative backends for [`winit`] to be constructed.
//!
//! [`winit`]: https://docs.rs/winit

#[cfg(any(not(feature = "std"), not(feature = "alloc")))]
compile_error! { "no-std and no-alloc usage are not yet supported" }

pub mod error;
