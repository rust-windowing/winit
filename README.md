# winit - Cross-platform window creation and management in Rust

[![Crates.io](https://img.shields.io/crates/v/winit.svg)](https://crates.io/crates/winit)
[![Docs.rs](https://docs.rs/winit/badge.svg)](https://docs.rs/winit)
[![UNSTABLE docs](https://img.shields.io/github/actions/workflow/status/rust-windowing/winit/docs.yml?branch=master&label=UNSTABLE%20docs
)](https://rust-windowing.github.io/winit/winit/index.html)
[![CI Status](https://github.com/rust-windowing/winit/workflows/CI/badge.svg)](https://github.com/rust-windowing/winit/actions)

```toml
[dependencies]
winit = "0.30.10"
```

## [Documentation](https://docs.rs/winit)

For features _within_ the scope of winit, see [FEATURES.md](FEATURES.md).

For features _outside_ the scope of winit, see [Are we GUI Yet?](https://areweguiyet.com/) and [Are we game yet?](https://arewegameyet.rs/), depending on what kind of project you're looking to do.

## Contact Us

Join us in our [![Matrix](https://img.shields.io/badge/Matrix-%23rust--windowing%3Amatrix.org-blueviolet.svg)](https://matrix.to/#/#rust-windowing:matrix.org) room.

The maintainers have a meeting every friday at UTC 15. The meeting notes can be found [here](https://hackmd.io/@winit-meetings).

## Usage

Winit is a window creation and management library. It can create windows and lets you handle
events (for example: the window being resized, a key being pressed, a mouse movement, etc.)
produced by the window.

Winit is designed to be a low-level brick in a hierarchy of libraries. Consequently, in order to
show something on the window you need to use the platform-specific getters provided by winit, or
another library.

## CONTRIBUTING

For contributing guidelines see [CONTRIBUTING.md](./CONTRIBUTING.md).

## MSRV Policy

This crate's Minimum Supported Rust Version (MSRV) is **1.80**. Changes to
the MSRV will be accompanied by a minor version bump.

As a **tentative** policy, the upper bound of the MSRV is given by the following
formula:

```
min(sid, stable - 3)
```

Where `sid` is the current version of `rustc` provided by [Debian Sid], and
`stable` is the latest stable version of Rust. This bound may be broken in case of a major ecosystem shift or a security vulnerability.

[Debian Sid]: https://packages.debian.org/sid/rustc

An exception is made for the Android platform, where a higher Rust version
must be used for certain Android features. In this case, the MSRV will be
capped at the latest stable version of Rust minus three. This inconsistency is
not reflected in Cargo metadata, as it is not powerful enough to expose this
restriction.

Redox OS is also not covered by this MSRV policy, as it requires a Rust nightly
toolchain to compile.

All crates in the [`rust-windowing`] organizations have the
same MSRV policy.

[`rust-windowing`]: https://github.com/rust-windowing

### Platform-specific usage

Check out the [`winit::platform`](https://docs.rs/winit/latest/winit/platform/index.html) module for platform-specific usage.

### Repository License

Note that the license in `LICENSE` doesn't apply in full to the DPI package [./dpi](./dpi).
Full details can be found in that folder's README.
<!-- This doesn't apply to users of the Winit crate, but this is also the repository level README -->
