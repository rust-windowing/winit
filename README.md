# winit - Cross-platform window creation and management in Rust

[![](http://meritbadge.herokuapp.com/winit)](https://crates.io/crates/winit)

[![Docs.rs](https://docs.rs/winit/badge.svg)](https://docs.rs/winit)

[![Build Status](https://travis-ci.org/tomaka/winit.png?branch=master)](https://travis-ci.org/tomaka/winit)
[![Build status](https://ci.appveyor.com/api/projects/status/5h87hj0g4q2xe3j9/branch/master?svg=true)](https://ci.appveyor.com/project/tomaka/winit/branch/master)

```toml
[dependencies]
winit = "0.7"
```

## [Documentation](https://docs.rs/winit)

## Usage

Winit is a window creation and management library. It can create windows and lets you handle
events (for example: the window being resized, a key being pressed, a mouse mouvement, etc.)
produced by window.

Winit is designed to be a low-level brick in a hierarchy of libraries. Consequently, in order to
show something on the window you need to use the platform-specific getters provided by winit, or
another library.

```rust
extern crate winit;

fn main() {
    let mut events_loop = winit::EventsLoop::new();
    let window = winit::Window::new(&events_loop).unwrap();

    events_loop.run_forever(|event| {
        match event {
            winit::Event::WindowEvent { event: winit::WindowEvent::Closed, .. } => {
                winit::ControlFlow::Break
            },
            _ => winit::ControlFlow::Continue,
        }
    });
}
```
