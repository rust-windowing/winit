//! # Changelog and migrations
//!
//! All notable changes to this project will be documented in this module,
//! along with migration instructions for larger changes.
// Put the current entry at the top of this page, for discoverability.
// See `.cargo/config.toml` for details about `unreleased_changelogs`.
#![cfg_attr(unreleased_changelogs, doc = include_str!("unreleased.md"))]
#![cfg_attr(not(unreleased_changelogs), doc = include_str!("v0.30.md"))]

#[doc = include_str!("v0.30.md")]
pub mod v0_30 {}

#[doc = include_str!("v0.29.md")]
pub mod v0_29 {}

#[doc = include_str!("v0.28.md")]
pub mod v0_28 {}

#[doc = include_str!("v0.27.md")]
pub mod v0_27 {}

#[doc = include_str!("v0.26.md")]
pub mod v0_26 {}

#[doc = include_str!("v0.25.md")]
pub mod v0_25 {}

#[doc = include_str!("v0.24.md")]
pub mod v0_24 {}

#[doc = include_str!("v0.23.md")]
pub mod v0_23 {}

#[doc = include_str!("v0.22.md")]
pub mod v0_22 {}

#[doc = include_str!("v0.21.md")]
pub mod v0_21 {}

#[doc = include_str!("v0.20.md")]
pub mod v0_20 {}

#[doc = include_str!("v0.19.md")]
pub mod v0_19 {}

#[doc = include_str!("v0.18.md")]
pub mod v0_18 {}

#[doc = include_str!("v0.17.md")]
pub mod v0_17 {}

#[doc = include_str!("v0.16.md")]
pub mod v0_16 {}

#[doc = include_str!("v0.15.md")]
pub mod v0_15 {}

#[doc = include_str!("v0.14.md")]
pub mod v0_14 {}

#[doc = include_str!("v0.13.md")]
pub mod v0_13 {}

#[doc = include_str!("v0.12.md")]
pub mod v0_12 {}

#[doc = include_str!("v0.11.md")]
pub mod v0_11 {}

#[doc = include_str!("v0.10.md")]
pub mod v0_10 {}

#[doc = include_str!("v0.9.md")]
pub mod v0_9 {}

#[doc = include_str!("v0.8.md")]
pub mod v0_8 {}
