//! Simple winit window example that prints keyboard events:
//! [KeyboardInput](https://docs.rs/winit/latest/winit/event/enum.WindowEvent.html#variant.KeyboardInput)
//! [ModifiersChanged](https://docs.rs/winit/latest/winit/event/enum.WindowEvent.html#variant.ModifiersChanged).)

use std::error::Error;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
#[cfg(web_platform)]
use winit::platform::web::WindowAttributesWeb;
use winit::window::{Window, WindowAttributes, WindowId};

#[path = "util/fill.rs"]
mod fill;
#[path = "util/tracing.rs"]
mod tracing;

#[derive(Default, Debug)]
struct App {
    window: Option<Box<dyn Window>>,
}

// https://docs.rs/winit/latest/winit/event/struct.Modifiers.html
// pub struct KeyEvent
// physical_key: PhysicalKey, enum PhysicalKey
//  Code        (       KeyCode)
// �Unidentified(NativeKeyCode)
// logical_key: Key, enum Key<Str = SmolStr>
//  Named(NamedKey)
//  Character(Str)
// �Unidentified(NativeKey)
// 🕱Dead(Option<char>)
//  text    : Option<SmolStr>
//  location: KeyLocation, enum KeyLocation Standard,Left,Right,Numpad
//  state   : ElementState, pressed/released
//🔁repeat  : bool
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, KeyLocation, PhysicalKey};
pub fn ev_key_s(key: &KeyEvent) -> String {
    let mut s = String::new();
    match &key.state {
        ElementState::Pressed => s.push('↓'),
        ElementState::Released => s.push('↑'),
    }
    if key.repeat {
        s.push('🔁')
    } else {
        s.push(' ')
    }; //𜱣⚛
    s.push(' ');
    match &key.physical_key {
        PhysicalKey::Code(key_code) => s.push_str(&format!("{:?}", key_code)),
        PhysicalKey::Unidentified(key_code_native) => {
            s.push_str(&format!("�{:?}", key_code_native))
        },
    };
    s.push(' ');
    match &key.logical_key {
        Key::Named(key_named) => s.push_str(&format!("{:?}", key_named)),
        Key::Character(key_char) => s.push_str(&format!("{}", key_char)),
        Key::Unidentified(key_native) => s.push_str(&format!("�{:?}", key_native)),
        Key::Dead(maybe_char) => s.push_str(&format!("🕱{:?}", maybe_char)),
    };
    s.push_str("  ");
    if let Some(txt) = &key.text {
        s.push_str(&format!("{}", txt));
    } else {
        s.push(' ');
    }
    s.push(' ');
    if let Some(txt) = &key.text_with_all_modifiers {
        s.push_str(&format!("{}", txt));
    } else {
        s.push(' ');
    }
    s.push(' ');
    match &key.key_without_modifiers {
        Key::Named(key_named) => s.push_str(&format!("{:?}", key_named)),
        Key::Character(key_char) => s.push_str(&format!("{}", key_char)),
        Key::Unidentified(key_native) => s.push_str(&format!("�{:?}", key_native)),
        Key::Dead(maybe_char) => s.push_str(&format!("🕱{:?}", maybe_char)),
    };
    s.push_str("  ");
    match &key.location {
        KeyLocation::Standard => s.push('≝'),
        KeyLocation::Left => s.push('←'),
        KeyLocation::Right => s.push('→'),
        KeyLocation::Numpad => s.push('🔢'),
    }
    s
}

impl ApplicationHandler for App {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        #[cfg(not(web_platform))]
        let window_attributes = WindowAttributes::default();
        #[cfg(web_platform)]
        let window_attributes = WindowAttributes::default()
            .with_platform_attributes(Box::new(WindowAttributesWeb::default().with_append(true)));
        self.window = match event_loop.create_window(window_attributes) {
            Ok(window) => Some(window),
            Err(err) => {
                eprintln!("error creating window: {err}");
                event_loop.exit();
                return;
            },
        }
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::ModifiersChanged(mods) => {
                println!("Δ {mods:#}\tmodifier state");
            },
            WindowEvent::KeyboardInput { event, is_synthetic, .. } => {
                let is_synthetic_s = if is_synthetic { "⚗" } else { " " };
                let key_event_s = ev_key_s(&event);
                println!("🖮 {}{}", is_synthetic_s, key_event_s);
            },
            WindowEvent::CloseRequested => {
                event_loop.exit();
            },
            WindowEvent::SurfaceResized(_) => {
                self.window.as_ref().expect("resize event without a window").request_redraw();
            },
            WindowEvent::RedrawRequested => {
                // Redraw the application.
                //
                // It's preferable for applications that do not render continuously to render in
                // this event rather than in AboutToWait, since rendering in here allows
                // the program to gracefully handle redraws requested by the OS.

                let window = self.window.as_ref().expect("redraw request without a window");

                // Notify that you're about to draw.
                window.pre_present_notify();

                // Draw.
                fill::fill_window(window.as_ref());

                // For contiguous redraw loop you can request a redraw from here.
                // window.request_redraw();
            },
            _ => (),
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(web_platform)]
    console_error_panic_hook::set_once();

    tracing::init();

    let event_loop = EventLoop::new()?;

    println!(
        "Δ is ModifiersChanged event, showing (line #1) side-agnostic modifier state as well as \
         (#2) side-aware one.\n   ⇧ Shift  ⎈ Control  ◆ Meta  ⎇ Alt  ⎇Gr AltGraph  ⇪ CapsLock  ⇭ \
         NumLock  ⇳🔒 ScrollLock\n   ƒ Fn  ƒ🔒 FnLock  カナ🔒 KanaLock  ‹👍 Loya  👍› Roya  🔣 \
         Symbol  🔣🔒 SymbolLock\n🖮 is KeyboardInput: ⚗ synthetic, ↓↑ pressed/unknown, 🔁 \
         repeat\n   phys logic txt +mod −mod location"
    );

    // For alternative loop run options see `pump_events` and `run_on_demand` examples.
    event_loop.run_app(App::default())?;

    Ok(())
}
