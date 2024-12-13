# Winit Scope

Winit aims to expose an interface that abstracts over window creation and input handling and can
be used to create both games and applications. It supports the following main graphical platforms:
- Desktop
  - Windows
  - macOS
  - Unix
    - via X11
    - via Wayland
  - Redox OS, via Orbital
- Mobile
  - iOS
  - Android
- Web

Most platforms expose capabilities that cannot be meaningfully transposed onto others. Winit does not
aim to support every single feature of every platform, but rather to abstract over the common features
available everywhere. In this context, APIs exposed in winit can be split into different "support tiers":

- **Core:** Features that are essential to providing a well-formed abstraction over each platform's
  windowing and input APIs.
- **Platform:** Platform-specific features that can't be meaningfully exposed through a common API and
  cannot be implemented outside of Winit without exposing a significant amount of Winit's internals
  or interfering with Winit's abstractions.
- **Usability:** Features that are not strictly essential to Winit's functionality, but provide meaningful
  usability improvements and cannot be reasonably implemented in an external crate. These are
  generally optional and exposed through Cargo features.

Core features are taken care of by the core Winit maintainers. Platform features are not.
When a platform feature is submitted, the submitter is considered the expert in the
feature and may be asked to support the feature should it break in the future.

Winit ***does not*** directly expose functionality for drawing inside windows or creating native
menus, but ***does*** commit to providing APIs that higher-level crates can use to implement that
functionality.

## `1.0` and stability

When all core features are implemented to the satisfaction of the Winit maintainers, Winit 1.0 will
be released and the library will enter maintenance mode. For the most part, new core features will not
be added past this point. New platform features may be accepted and exposed through point releases.

### Tier upgrades
Some platform features could, in theory, be exposed across multiple platforms, but have not gone
through the implementation work necessary to function on all platforms. When one of these features
gets implemented across all platforms, a PR can be opened to upgrade the feature to a core feature.
If that gets accepted, the platform-specific functions get deprecated and become permanently
exposed through the core, cross-platform API.
