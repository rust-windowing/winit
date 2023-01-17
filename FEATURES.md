# Winit Scope

Winit aims to expose an interface that abstracts over window creation and input handling, and can
be used to create both games and applications. It supports the following main graphical platforms:
- Desktop
  - Windows 7+ (10+ is tested regularly)
  - macOS 10.7+ (10.14+ is tested regularly)
  - Unix
    - via X11
    - via Wayland
  - Redox OS, via Orbital
- Mobile
  - iOS
  - Android
- Web
  - via WASM

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
Some platform features could in theory be exposed across multiple platforms, but have not gone
through the implementation work necessary to function on all platforms. When one of these features
gets implemented across all platforms, a PR can be opened to upgrade the feature to a core feature.
If that gets accepted, the platform-specific functions gets deprecated and become permanently
exposed through the core, cross-platform API.

# Features

## Extending this section

If your PR makes notable changes to Winit's features, please update this section as follows:

- If your PR adds a new feature, add a brief description to the relevant section. If the feature is a core
  feature, add a row to the feature matrix and describe what platforms the feature has been implemented on.

- If your PR begins a new API rework, add a row to the `Pending API Reworks` table. If the PR implements the
  API rework on all relevant platforms, please move it to the `Completed API Reworks` table.

- If your PR implements an already-existing feature on a new platform, either mark the feature as *completed*,
  or mark it as *mostly completed* and link to an issue describing the problems with the implementation.

## Core

### Windowing
- **Window initialization**: Winit allows the creation of a window
- **Providing pointer to init OpenGL**: Winit provides the necessary pointers to initialize a working opengl context
- **Providing pointer to init Vulkan**: Same as OpenGL but for Vulkan
- **Window decorations**: The windows created by winit are properly decorated, and the decorations can
  be deactivated
- **Window decorations toggle**: Decorations can be turned on or off after window creation
- **Window resizing**: The windows created by winit can be resized and generate the appropriate events
  when they are. The application can precisely control its window size if desired.
- **Window resize increments**: When the window gets resized, the application can choose to snap the window's
  size to specific values.
- **Window transparency**: Winit allows the creation of windows with a transparent background.
- **Window maximization**: The windows created by winit can be maximized upon creation.
- **Window maximization toggle**: The windows created by winit can be maximized and unmaximized after
  creation.
- **Window minimization**: The windows created by winit can be minimized after creation.
- **Fullscreen**: The windows created by winit can be put into fullscreen mode.
- **Fullscreen toggle**: The windows created by winit can be switched to and from fullscreen after
  creation.
- **Exclusive fullscreen**: Winit allows changing the video mode of the monitor
  for fullscreen windows, and if applicable, captures the monitor for exclusive
  use by this application.
- **HiDPI support**: Winit assists developers in appropriately scaling HiDPI content.
- **Popup / modal windows**: Windows can be created relative to the client area of other windows, and parent
  windows can be disabled in favor of popup windows. This feature also guarantees that popup windows
  get drawn above their owner.


### System Information
- **Monitor list**: Retrieve the list of monitors and their metadata, including which one is primary.
- **Video mode query**: Monitors can be queried for their supported fullscreen video modes (consisting of resolution, refresh rate, and bit depth).

### Input Handling
- **Mouse events**: Generating mouse events associated with pointer motion, click, and scrolling events.
- **Mouse set location**: Forcibly changing the location of the pointer.
- **Cursor locking**: Locking the cursor inside the window so it cannot move.
- **Cursor confining**: Confining the cursor to the window bounds so it cannot leave them.
- **Cursor icon**: Changing the cursor icon, or hiding the cursor.
- **Cursor hittest**: Handle or ignore mouse events for a window.
- **Touch events**: Single-touch events.
- **Touch pressure**: Touch events contain information about the amount of force being applied.
- **Multitouch**: Multi-touch events, including cancellation of a gesture.
- **Keyboard events**: Properly processing keyboard events using the user-specified keymap and
  translating keypresses into UTF-8 characters, handling dead keys and IMEs.
- **Drag & Drop**: Dragging content into winit, detecting when content enters, drops, or if the drop is cancelled.
- **Raw Device Events**: Capturing input from input devices without any OS filtering.
- **Gamepad/Joystick events**: Capturing input from gamepads and joysticks.
- **Device movement events**: Capturing input from the device gyroscope and accelerometer.

## Platform
### Windows
* Setting the taskbar icon
* Setting the parent window
* Setting a menu bar
* `WS_EX_NOREDIRECTIONBITMAP` support
* Theme the title bar according to Windows 10 Dark Mode setting or set a preferred theme

### macOS
* Window activation policy
* Window movable by background
* Transparent titlebar
* Hidden titlebar
* Hidden titlebar buttons
* Full-size content view
* Accepts first mouse
* Set a preferred theme and get current theme.

### Unix
* Window urgency
* X11 Window Class
* X11 Override Redirect Flag
* GTK Theme Variant
* Base window size
* Setting the X11 parent window

### iOS
* `winit` has a minimum OS requirement of iOS 8
* Get the `UIWindow` object pointer
* Get the `UIViewController` object pointer
* Get the `UIView` object pointer
* Get the `UIScreen` object pointer
* Setting the `UIView` hidpi factor
* Valid orientations
* Home indicator visibility
* Status bar visibility
* Deferrring system gestures
* Getting the device idiom
* Getting the preferred video mode

### Web
* Get if systems preferred color scheme is "dark"

## Usability
* `serde`: Enables serialization/deserialization of certain types with Serde. (Maintainer: @Osspial)

## Compatibility Matrix

Legend:

- ✔️: Works as intended
- ▢: Mostly works but some bugs are known
- ❌: Missing feature or large bugs making it unusable
- **N/A**: Not applicable for this platform
- ❓: Unknown status

### Windowing
|Feature                          |Windows|MacOS   |Linux x11   |Linux Wayland  |Android|iOS    |WASM      |Redox OS|
|-------------------------------- | ----- | ----   | -------    | -----------   | ----- | ----- | -------- | ------ |
|Window initialization            |✔️     |✔️     |▢[#5]      |✔️             |▢[#33]|▢[#33] |✔️        |✔️      |
|Providing pointer to init OpenGL |✔️     |✔️     |✔️         |✔️             |✔️     |✔️    |**N/A**|✔️      |
|Providing pointer to init Vulkan |✔️     |✔️     |✔️         |✔️             |✔️     |❓     |**N/A**|**N/A** |
|Window decorations               |✔️     |✔️     |✔️         |✔️             |**N/A**|**N/A**|**N/A**|✔️      |
|Window decorations toggle        |✔️     |✔️     |✔️         |✔️             |**N/A**|**N/A**|**N/A**|**N/A** |
|Window resizing                  |✔️     |▢[#219]|✔️         |▢[#306]        |**N/A**|**N/A**|✔️        |✔️      |
|Window resize increments         |❌     |✔️     |✔️         |❌             |**N/A**|**N/A**|**N/A**|**N/A** |
|Window transparency              |✔️     |✔️     |✔️         |✔️             |**N/A**|**N/A**|N/A        |✔️      |
|Window maximization              |✔️     |✔️     |✔️         |✔️             |**N/A**|**N/A**|**N/A**|**N/A** |
|Window maximization toggle       |✔️     |✔️     |✔️         |✔️             |**N/A**|**N/A**|**N/A**|**N/A** |
|Window minimization              |✔️     |✔️     |✔️         |✔️             |**N/A**|**N/A**|**N/A**|**N/A** |
|Fullscreen                       |✔️     |✔️     |✔️         |✔️             |**N/A**|✔️     |✔️        |**N/A** |
|Fullscreen toggle                |✔️     |✔️     |✔️         |✔️             |**N/A**|✔️     |✔️        |**N/A** |
|Exclusive fullscreen             |✔️     |✔️     |✔️         |**N/A**         |❌    |✔️     |**N/A**|**N/A** |
|HiDPI support                    |✔️     |✔️     |✔️         |✔️             |✔️     |✔️    |✔️    |❌      |
|Popup windows                    |❌     |❌     |❌         |❌             |❌    |❌     |**N/A**|**N/A** |

### System information
|Feature          |Windows|MacOS |Linux x11|Linux Wayland|Android|iOS      |WASM      |Redox OS|
|---------------- | ----- | ---- | ------- | ----------- | ----- | ------- | -------- | ------ |
|Monitor list     |✔️    |✔️    |✔️       |✔️          |✔️     |✔️      |**N/A**|❌      |
|Video mode query |✔️    |✔️    |✔️       |✔️          |✔️     |✔️      |**N/A**|❌      |

### Input handling
|Feature                 |Windows   |MacOS   |Linux x11|Linux Wayland|Android|iOS    |WASM      |Redox OS|
|----------------------- | -----    | ----   | ------- | ----------- | ----- | ----- | -------- | ------ |
|Mouse events            |✔️       |▢[#63]  |✔️       |✔️          |**N/A**|**N/A**|✔️        |✔️      |
|Mouse set location      |✔️       |✔️      |✔️       |✔️(when locked)  |**N/A**|**N/A**|**N/A**|**N/A** |
|Cursor locking          |❌       |✔️      |❌       |✔️          |**N/A**|**N/A**|✔️        |❌      |
|Cursor confining        |✔️       |❌      |✔️       |✔️          |**N/A**|**N/A**|❌       |❌      |
|Cursor icon             |✔️       |✔️      |✔️       |✔️          |**N/A**|**N/A**|✔️        |**N/A** |
|Cursor hittest          |✔️       |✔️      |❌       |✔️          |**N/A**|**N/A**|❌        |❌      |
|Touch events            |✔️       |❌      |✔️       |✔️          |✔️    |✔️     |✔️        |**N/A** |
|Touch pressure          |✔️       |❌      |❌       |❌          |❌    |✔️     |✔️        |**N/A** |
|Multitouch              |✔️       |❌      |✔️       |✔️          |✔️    |✔️     |❌        |**N/A** |
|Keyboard events         |✔️       |✔️      |✔️       |✔️          |✔️    |❌     |✔️        |✔️      |
|Drag & Drop             |▢[#720]  |▢[#720] |▢[#720]  |❌[#306]    |**N/A**|**N/A**|❓        |**N/A** |
|Raw Device Events       |▢[#750]  |▢[#750] |▢[#750]  |❌          |❌    |❌     |❓        |**N/A** |
|Gamepad/Joystick events |❌[#804] |❌      |❌       |❌          |❌    |❌     |❓        |**N/A** |
|Device movement events  |❓        |❓       |❓       |❓           |❌    |❌     |❓        |**N/A** |
|Drag window with cursor |✔️       |✔️      |✔️       |✔️          |**N/A**|**N/A**|**N/A**   |**N/A** |
|Resize with cursor      |❌         |❌       |✔️       |❌       |**N/A**|**N/A**|**N/A**   |**N/A** |

### Pending API Reworks
Changes in the API that have been agreed upon but aren't implemented across all platforms.

|Feature                             |Windows|MacOS |Linux x11|Linux Wayland|Android|iOS    |WASM      |Redox OS|
|------------------------------      | ----- | ---- | ------- | ----------- | ----- | ----- | -------- | ------ |
|New API for HiDPI ([#315] [#319])   |✔️    |✔️    |✔️       |✔️          |✔️     |✔️    |❓        |❓      |
|Event Loop 2.0 ([#459])             |✔️    |✔️    |❌       |✔️          |✔️     |✔️    |❓        |❓      |
|Keyboard Input ([#812])             |❌    |❌    |❌       |❌          |❌     |❌    |❓        |❓      |

### Completed API Reworks
|Feature                             |Windows|MacOS |Linux x11|Linux Wayland|Android|iOS    |WASM      |Redox OS|
|------------------------------      | ----- | ---- | ------- | ----------- | ----- | ----- | -------- | ------ |

[#165]: https://github.com/rust-windowing/winit/issues/165
[#219]: https://github.com/rust-windowing/winit/issues/219
[#242]: https://github.com/rust-windowing/winit/issues/242
[#306]: https://github.com/rust-windowing/winit/issues/306
[#315]: https://github.com/rust-windowing/winit/issues/315
[#319]: https://github.com/rust-windowing/winit/issues/319
[#33]: https://github.com/rust-windowing/winit/issues/33
[#459]: https://github.com/rust-windowing/winit/issues/459
[#5]: https://github.com/rust-windowing/winit/issues/5
[#63]: https://github.com/rust-windowing/winit/issues/63
[#720]: https://github.com/rust-windowing/winit/issues/720
[#721]: https://github.com/rust-windowing/winit/issues/721
[#750]: https://github.com/rust-windowing/winit/issues/750
[#804]: https://github.com/rust-windowing/winit/issues/804
[#812]: https://github.com/rust-windowing/winit/issues/812
