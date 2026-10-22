# winit - Cross-platform window creation and management in Rust

[![Crates.io](https://img.shields.io/crates/v/winit.svg)](https://crates.io/crates/winit)
[![Docs.rs](https://docs.rs/winit/badge.svg)](https://docs.rs/winit)
[![Master Docs](https://img.shields.io/github/actions/workflow/status/rust-windowing/winit/docs.yml?branch=master&label=master%20docs
)](https://rust-windowing.github.io/winit/winit/index.html)
[![CI Status](https://github.com/rust-windowing/winit/workflows/CI/badge.svg)](https://github.com/rust-windowing/winit/actions)

```toml
[dependencies]
winit = "0.30.13"
```

## [Documentation](https://docs.rs/winit)

For features _within_ the scope of winit, see [FEATURES.md](FEATURES.md).

For features _outside_ the scope of winit, see [Are we GUI Yet?](https://areweguiyet.com/) and [Are we game yet?](https://arewegameyet.rs/), depending on what kind of project you're looking to do.

## Native Wayland file drops

The 0.30 Wayland backend accepts local `text/uri-list` drops through the existing
`WindowEvent::DroppedFile` API. Enable `wayland-portal` for
`application/vnd.portal.filetransfer` offers (including Flatpak); the portal grants
access only to the dropped files, without broad filesystem permissions. The portal
feature uses zbus 5 and requires Rust 1.87; applications that do not enable this new
feature retain the existing feature set and MSRV policy.

Only Copy drops are accepted. Transfers are asynchronous, capped at 1 MiB of
metadata and 256 paths, with at most eight pending drops and a ten-second deadline.
No file names or portal capabilities are fetched while hovering, so
`HoveredFile` is not emitted on Wayland. Clipboard ownership stays with the
application's clipboard integration.

Protocol regressions use a private server, without an existing desktop:
`cargo test --features wayland-portal --test wayland_dnd`.
The ignored `native_file_drop_portal` test additionally needs a session bus with
`org.freedesktop.portal.Documents`; it creates only temporary test transfers.

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

This crate's Minimum Supported Rust Version (MSRV) is **1.70**. Changes to
the MSRV will be accompanied by a minor version bump.

As a **tentative** policy, the upper bound of the MSRV is given by the following
formula:

```
min(sid, stable - 3)
```

Where `sid` is the current version of `rustc` provided by [Debian Sid], and
`stable` is the latest stable version of Rust. This bound may be broken in case of a major ecosystem shift or a security vulnerability.

[Debian Sid]: https://packages.debian.org/sid/rustc

The exception is for the Android platform, where a higher Rust version
must be used for certain Android features. In this case, the MSRV will be
capped at the latest stable version of Rust minus three. This inconsistency is
not reflected in Cargo metadata, as it is not powerful enough to expose this
restriction.

All crates in the [`rust-windowing`] organizations have the
same MSRV policy.

[`rust-windowing`]: https://github.com/rust-windowing

### Platform-specific usage

Check out the [`winit::platform`](https://rust-windowing.github.io/winit/winit/platform/index.html) module for platform-specific usage.
