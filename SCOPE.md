# Winit Scope

Winit aims to expose an interface that abstracts over window creation and input handling, and can
be used to create both games and applications. It supports the main graphical platforms:
- Desktop
  - Windows
  - macOS
  - Unix
    - via X11
    - via Wayland
- Mobile
  - iOS
  - Android
- Web
  - via Emscripten
  - via WASM ***// TODO: DO WE ACTUALLY WANT TO SUPPORT THIS?***

Most platforms expose capabilities that cannot be meaningfully transposed to the others. Winit does not
aim to support every single functionality of every platform, but rather to abstract the set of
capabilities that is common to all platforms. In this context, APIs exposed in winit can be split into
different "support levels":

- Tier 1: Features that are essential to providing a well-formed abstraction over each platform's
  windowing and input APIs.
- Tier 2: Platform-specific features. There are two classes of Tier 2 features:
  - Tier 2A: Features that can't be meaningfully exposed through a common API and
    cannot be implemented outside of Winit without exposing a significant amount of Winit's internals
    or interfering with Winit's abstractions.
  - Tier 2B: Features that could in theory be exposed across multiple platforms, but have not gone
    through the implementation work necessary to function on all platforms. Winit provides no
    stability guarantees for these features.
- Tier 3: Features that are not strictly essential to Winit's functionality, but provide meaningful
  usability improvements and cannot be reasonably implemented in an external crate. These are
  generally optional and exposed through Cargo features.

Tier 1 features are taken care of by the core Winit maintainers. Tier 2 and 3 features are not. When
a Tier 2 or 3 feature is submitted, the submitter is considered the expert in the feature and may be
asked to support the feature should it break in the future.

Winit ***does not*** directly expose functionality for drawing inside windows or creating native
menus, but ***does*** commit to providing APIs that higher-level crates can use to implement that
functionality. When requesting or implementing a new Winit feature, you should consider whether or
not it's directly related to window creation or input handling. If it isn't, it may be worth creating
a separate crate that extends Winit's API to add that functionality.

## Tier upgrades
In some cases, Tier 2 features get implemented across enough platforms that a common API *can* be
exposed. When this happens, these features may be moved out of their platform module and into the
core Winit API.

## The `1.0` release
When all Tier 1 features are implemented to the satisfaction of the Winit maintainers, Winit will
enter maintenance mode. New Tier 1 features will, for the most part, not be added past this point.
New Tier 2 and 3 features may be accepted and exposed through point releases.

# Features

## Tier 1
Each section includes a collapsed description of the features it lists.

Legend:

- ✔️: Works as intended
- ▢: Mostly works but some bugs are known
- ❌: Missing feature or large bugs making it unusable
- **N/A**: Not applicable for this platform
- ❓: Unknown status


### Windowing
|Feature                          |Windows|MacOS |Linux x11|Linux Wayland|Android|iOS    |Emscripten|
|-------------------------------- | ----- | ---- | ------- | ----------- | ----- | ----- | -------- |
|Window initialization            |✔️    |✔️    |▢#5      |✔️          |▢#33   |▢#33  |❓        |
|Providing pointer to init OpenGL |✔️    |✔️    |✔️       |✔️          |✔️     |✔️    |❓        |
|Providing pointer to init Vulkan |✔️    |✔️    |✔️       |✔️          |✔️     |❓     |**N/A**   |
|Window decorations               |✔️    |✔️    |✔️       |▢#306       |**N/A**|**N/A**|**N/A**   |
|Window decorations toggle        |✔️    |✔️    |✔️       |✔️          |**N/A**|**N/A**|**N/A**   |
|Window resizing                  |✔️    |▢#219 |✔️       |▢#306       |**N/A**|**N/A**|❓        |
|Window transparency              |✔️    |✔️    |✔️       |✔️          |**N/A**|**N/A**|**N/A**   |
|Window maximization              |✔️    |✔️    |✔️       |✔️          |**N/A**|**N/A**|**N/A**   |
|Window maximization toggle       |✔️    |✔️    |✔️       |✔️          |**N/A**|**N/A**|**N/A**   |
|Fullscreen                       |✔️    |✔️    |✔️       |✔️          |**N/A**|**N/A**|❌        |
|Fullscreen toggle                |✔️    |✔️    |✔️       |✔️          |**N/A**|**N/A**|❌        |
|HiDPI support #105               |✔️    |✔️    |✔️       |✔️          |▢      |✔️    |✔️        |
|Child windows ***//DISCUSS***    |❌    |❌    |❌       |❌          |❌    |❌     |❌        |

#### Details
- **Window initialization**: Winit allows the creation of a window
- **Pointer to OpenGL**: Winit provides the necessary pointers to initialize a working opengl context
- **Pointer to Vulkan**: Same as OpenGL but for Vulkan
- **Window decorations**: The windows created by winit are properly decorated, and the decorations can
  be deactivated
- **Window decorations toggle**: Decorations can be turned on or off after window creation
- **Window resizing**: The windows created by winit can be resized and generate the appropriate events
  when they are. The application can precisely control its window size if wanted.
- **Window transaprency**: Winit allows the creation of windows with a transparent background.
- **Window maximization**: The windows created by winit can be maximized upon creation.
- **Window maximization toggle**: The windows created by winit can be maximized and unmaximized after
  creation.
- **Fullscreen**: The windows created by winit support being fullscreen.
- **Fullscreen toggle**: The windows created by winit can be switched to and from fullscreen after
  creation.
- **Child windows**: Windows can be created relative to the client area of other windows, and parent
  windows can be disabled in favor of child windows.

### System information
|Feature      |Windows|MacOS |Linux x11|Linux Wayland|Android|iOS    |Emscripten|
|------------ | ----- | ---- | ------- | ----------- | ----- | ----- | -------- |
|Monitor list |✔️    |✔️    |✔️       |✔️          |**N/A**|**N/A**|**N/A**   |

#### Details
- **Monitor list**: Retrieve the list of monitors and their metada, including which one is primary is applicable

### Input handling
|Feature                                 |Windows|MacOS |Linux x11|Linux Wayland|Android|iOS    |Emscripten|
|--------------------------------------- | ----- | ---- | ------- | ----------- | ----- | ----- | -------- |
|Mouse events                            |✔️    |▢ #63 |✔️       |✔️          |**N/A**|**N/A**|✔️       |
|Mouse set location                      |✔️    |✔️    |✔️       |❓           |**N/A**|**N/A**|**N/A**  |
|Cursor grab                             |✔️    |▢ #165|▢ #242   |❌ #306     |**N/A**|**N/A**|✔️       |
|Cursor icon                             |✔️    |✔️    |✔️       |❌ #306     |**N/A**|**N/A**|❌       |
|Touch events                            |✔️    |❌    |✔️       |✔️          |✔️    |✔️     |✔️       |
|Multitouch                              |❓     |❌    |✔️       |✔️          |❓     |❌     |❌       |
|Keyboard events                         |✔️    |✔️    |✔️       |✔️          |❓     |❌     |✔️       |
|Drag & Drop                             |✔️    |✔️    |✔️       |❌ #306     |❌    |❌     |❌       |
|Clipboard #162                          |❌    |❌    |❌       |❌          |❌    |❌     |❌       |
|Raw Device Events                       |▢     |▢     |▢        |❌          |❌    |❌     |❌       |
|Gamepad/Joystick events ***//DISCUSS*** |❌    |❌    |❌       |❌          |❌    |❌     |❌       |
|Device movement events ***//DISCUSS***  |❓     |❓     |❓       |❓           |❌    |❌     |❌       |

#### Details
- **Mouse events**: Generating mouse events associated with pointer motion, click, and scrolling events.
- **Mouse set location**: Forcibly changing the location of the pointer.
- **Cursor grab**: Locking the cursor so it cannot exit the client area of a window.
- **Cursor icon**: Changing the cursor icon, or hiding the cursor.
- **Touch events**: Single-touch events.
- **Multitouch**: Multi-touch events, including cancellation of a gesture.
- **Keyboard events**: Properly processing keyboard events using the user-specified keymap and
  translating keypresses into UTF-8 characters, handling dead keys and IMEs.
- **Drag & Drop**: Dragging content into winit, detecting when content enters, drops, or if the drop is cancelled.
  ***//DISCUSS: WINIT SUPPORTS FILE DROPS, BUT NOT TEXT OR IMAGE DROPS***
- **Clipboard**: Winit supports copy-pasting content to and from winit.
  ***//DISCUSS: AN ISSUE SHOULD BE OPENED REGARDING HOW THIS API SHOULD WORK***
- **Raw Device Events**: Capturing input from input devices without any OS filtering.
- **Gamepad/Joystick events**: Capturing input from gampads and joysticks.
  ***//DISCUSS: SHOULD THIS BE SUPPORTED?***
- **Device movement events:**: Capturing input from the device gyroscope and accelerometer.
  ***//DISCUSS: SHOULD THIS BE SUPPORTED?***

### Pending API Reworks
Here are listed Changes in the API that have been merged but are still stubbed out in some platforms
|Feature                         |Windows|MacOS |Linux x11|Linux Wayland|Android|iOS    |Emscripten|
|------------------------------  | ----- | ---- | ------- | ----------- | ----- | ----- | -------- |
|New API for HiDPI (#315 #319)   |✔️    |✔️    |✔️       |✔️          |▢      |✔️    |✔️        |
|Event Loop 2.0 (#459)           |❌#638|❌    |❌       |❌          |❌     |❌    |❌        |

## Tier 2
### Windows
* Setting the taskbar icon (Maintainer: ***???***)
* Setting the parent window (Maintainer: ***???***)
  ***//DISCUSS: SHOULD THIS BE SUBSUMED INTO A CORE CHILD WINDOW FEATURE?***
* `WS_EX_NOREDIRECTIONBITMAP` support (Maintainer: ***???***)

### macOS
* Window activation policy (Maintainer: ***???***)
* Window movable by background (Maintainer: ***???***)
* Transparent titlebar (Maintainer: ***???***)
* Hidden titlebar (Maintainer: ***???***)
* Hidden titlebar buttons (Maintainer: ***???***)
* Full-size content view (Maintainer: ***???***)
* Resize increments (Maintainer: ***???***)

### Unix
* Window urgency
* X11 Window Class
* X11 Override Redirect Flag
* GTK Theme Variant
* Resize increment
* Base window size

## Tier 3
* `icon_loading`: Enables loading window icons directly from files. (Maintainer: @francesca64)
* `serde`: Enables serialization/deserialization of certain types with Serde. (Maintainer: @Osspial)
