//! Showcases the use of an input method engine (IME)
//! by emulating a text edit field.
//!
//! Use CTRL+i to toggle IME support.
//! Use CTRL+p to cycle content purpose values.
//! Use CTRL+h to cycle content hint permutations.

use std::cmp;
use std::error::Error;

use ::tracing::{error, info};
use dpi::{LogicalPosition, PhysicalSize};
use winit::application::ApplicationHandler;
use winit::event::{Ime, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState};
#[cfg(web_platform)]
use winit::platform::web::WindowAttributesWeb;
use winit::window::{
    ImeCapabilities, ImeEnableRequest, ImeHint, ImePurpose, ImeRequest, ImeRequestData,
    ImeSurroundingText, Window, WindowAttributes, WindowId,
};

#[path = "util/fill.rs"]
mod fill;
#[path = "util/tracing.rs"]
mod tracing;

const IME_CURSOR_SIZE: PhysicalSize<u32> = PhysicalSize::new(20, 20);

#[derive(Debug)]
struct App {
    window: Option<Box<dyn Window>>,
    text: TextInputState,
    modifiers: ModifiersState,
}

/// State of the undisplayed text input field.
#[derive(Debug)]
struct TextInputState {
    ime_enabled: bool,
    /// The contents of the emulated text field for IME purposes (not displayed).
    /// (text, cursor position in bytes).
    contents: (String, usize),
    /// The purpose of the contents the emulated text field expects
    purpose: ImePurpose,
    /// The behaviour hints for the IME regarding the emulated text field
    hint: ImeHint,
}

impl TextInputState {
    fn append_text(&mut self, text: &str) {
        self.contents.0.push_str(text);
        self.contents.1 += text.len();
    }
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
        };

        // Allow IME out of the box.
        let enable_request = ImeEnableRequest::new(
            ImeCapabilities::new()
                .with_purpose()
                .with_hint()
                .with_cursor_area()
                .with_surrounding_text(),
            self.get_ime_update(),
        )
        .unwrap();
        let enable_ime = ImeRequest::Enable(enable_request);

        // Initial update
        self.window().request_ime_update(enable_ime).unwrap();
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                println!("Close was requested; stopping");
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
            WindowEvent::Ime(event) => {
                self.handle_ime_event(event);
            },
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
                info!("Modifiers changed to {:?}", self.modifiers);
            },
            WindowEvent::KeyboardInput { event, is_synthetic: false, .. } => {
                let mods = self.modifiers;

                // Dispatch actions only on press.
                if event.state.is_pressed() {
                    self.handle_key_pressed(event, mods);
                }
            },
            _ => (),
        }
    }
}

impl App {
    fn window(&self) -> &dyn Window {
        self.window.as_ref().unwrap().as_ref()
    }

    pub fn get_ime_update(&self) -> ImeRequestData {
        let (text, cursor) = &self.text.contents;
        // A rudimentary text field emulation: the caret moves right by a constant amount for each
        // code point.

        let text_before_caret = if text.is_char_boundary(*cursor) { &text[..*cursor] } else { "" };
        let chars_before_caret = text_before_caret.chars().count();
        let cursor_pos = LogicalPosition::<u32> { x: 10 * chars_before_caret as u32, y: 0 };

        // Limit text field size
        const MAX_BYTES: usize = ImeSurroundingText::MAX_TEXT_BYTES;
        let minimal_offset = cursor / MAX_BYTES * MAX_BYTES;
        let first_char_boundary =
            (minimal_offset..*cursor).find(|off| text.is_char_boundary(*off)).unwrap_or(*cursor);
        let last_char_boundary = (*cursor..(first_char_boundary + MAX_BYTES))
            .rev()
            .find(|off| text.is_char_boundary(*off))
            .unwrap_or(*cursor);
        let surrounding_text = &text[first_char_boundary..last_char_boundary];
        let relative_cursor = cursor - first_char_boundary;
        let surrounding_text =
            ImeSurroundingText::new(surrounding_text.into(), relative_cursor, relative_cursor)
                .expect("Bug in example: bad byte calculations");

        ImeRequestData::default()
            .with_purpose(self.text.purpose)
            .with_hint(self.text.hint)
            .with_cursor_area(cursor_pos.into(), IME_CURSOR_SIZE.into())
            .with_surrounding_text(surrounding_text)
    }

    pub fn toggle_ime(&mut self) {
        if self.text.ime_enabled {
            self.window().request_ime_update(ImeRequest::Disable).expect("disable can not fail");
        } else {
            let enable_request = ImeEnableRequest::new(
                ImeCapabilities::new()
                    .with_purpose()
                    .with_hint()
                    .with_cursor_area()
                    .with_surrounding_text(),
                self.get_ime_update(),
            )
            .unwrap();
            self.window().request_ime_update(ImeRequest::Enable(enable_request)).unwrap();
        };

        self.text.ime_enabled = !self.text.ime_enabled;
        info!("IME enabled now {}", self.text.ime_enabled);
    }

    fn handle_key_pressed(&mut self, event: winit::event::KeyEvent, mods: ModifiersState) {
        if mods == ModifiersState::CONTROL {
            if let Key::Character(ch) = event.key_without_modifiers.as_ref() {
                if ch == "i" {
                    self.toggle_ime();
                }
                if ch == "p" {
                    self.text.purpose = match self.text.purpose {
                        ImePurpose::Normal => ImePurpose::Password,
                        ImePurpose::Password => ImePurpose::Terminal,
                        ImePurpose::Terminal => ImePurpose::Number,
                        ImePurpose::Number => ImePurpose::Phone,
                        ImePurpose::Phone => ImePurpose::Url,
                        ImePurpose::Url => ImePurpose::Email,
                        ImePurpose::Email => ImePurpose::Pin,
                        ImePurpose::Pin => ImePurpose::Date,
                        ImePurpose::Date => ImePurpose::Time,
                        ImePurpose::Time => ImePurpose::DateTime,
                        ImePurpose::DateTime => ImePurpose::Normal,
                        _ => ImePurpose::Normal,
                    };
                    if self.text.ime_enabled {
                        self.window()
                            .request_ime_update(ImeRequest::Update(self.get_ime_update()))
                            .unwrap();
                    }
                    info!("text input purpose now {:?}", self.text.purpose);
                }
                if ch == "h" {
                    let bump = |hint: ImeHint| {
                        if hint.is_all() {
                            ImeHint::NONE
                        } else {
                            // Go through all integers. We'll skip invalid ones
                            ImeHint::from_bits_retain(hint.bits().wrapping_add(1))
                        }
                    };
                    let mut new_hint = bump(self.text.hint);

                    while !ImeHint::all().contains(new_hint) {
                        new_hint = bump(new_hint);
                    }
                    self.text.hint = new_hint;
                    if self.text.ime_enabled {
                        self.window()
                            .request_ime_update(ImeRequest::Update(self.get_ime_update()))
                            .unwrap();
                    }
                    info!("text input IME hint now {:?}", self.text.hint);
                }
            }
        } else if let Some(text) = event.text_with_all_modifiers {
            self.text.append_text(&text);
            self.window().request_ime_update(ImeRequest::Update(self.get_ime_update())).unwrap();
            info!("text input now contains:");
            let (text, cursor) = &self.text.contents;
            // Representing a selection with the cursor and anchor as ends is not
            // supported yet. Using the same position for anchor to mark no
            // selection.
            info!("{}", preedit_with_cursor(text, *cursor, *cursor));
        }
    }

    fn handle_ime_event(&mut self, event: Ime) {
        let window = self.window.as_ref().expect("IME request without a window");
        match event {
            Ime::Enabled => info!("IME enabled for Window={:?}", window.id()),
            Ime::Preedit(text, caret_pos) => {
                info!("Preedit: {}, with caret at {:?}", text, caret_pos);
            },
            Ime::Commit(text) => {
                self.text.append_text(&text);
                info!("Committed: {}", text);
                let request_data = self.get_ime_update();
                window.request_ime_update(ImeRequest::Update(request_data)).unwrap();
            },
            Ime::DeleteSurrounding { before_bytes, after_bytes } => {
                let (text, cursor) = &self.text.contents;

                // To anyone copying this, keep in mind that this doesn't take text
                // selection into account. The deletion happens
                // *around* the pre-edit, and may remove the whole
                // selection or a part of it.
                let delete_start = cursor.saturating_sub(before_bytes);
                let delete_end = cmp::min(cursor.saturating_add(after_bytes), text.len());
                if text.is_char_boundary(delete_start) && text.is_char_boundary(delete_end) {
                    let new_text = {
                        let mut t = String::from(&text[..delete_start]);
                        t.push_str(&text[delete_end..]);
                        t
                    };
                    self.text.contents = (new_text, delete_start);
                    info!("IME deleted bytes: {before_bytes}, {after_bytes}");
                } else {
                    error!("Buggy IME tried to delete with indices not on char boundary.");
                }
            },
            Ime::Disabled => info!("IME disabled for Window={:?}", window.id()),
        }
    }
}

/// Prints text of the text field, highlighting cursor position
fn preedit_with_cursor(text: &str, cursor: usize, anchor: usize) -> String {
    preedit_with_cursor_checked(text, cursor, anchor).unwrap_or_else(|e| format!("INVALID: {e}"))
}

fn preedit_with_cursor_checked(text: &str, cursor: usize, anchor: usize) -> Result<String, &str> {
    let first = cmp::min(cursor, anchor);
    let before = text.get(0..first).ok_or("first segment ends not on char boundary")?;
    let second = cmp::max(cursor, anchor);
    let mid = if second == first {
        None
    } else {
        Some(text.get(first..second).ok_or("second segment ends not on char boundary")?)
    };
    let end = text.get(second..).unwrap();
    Ok(match (first == cursor, before, mid, end) {
        (_, before, None, end) => format!("{before}|{end}"),
        (true, before, Some(mid), end) => format!("{before}|{mid}_{end}"),
        (false, before, Some(mid), end) => format!("{before}_{mid}|{end}"),
    })
}

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(web_platform)]
    console_error_panic_hook::set_once();

    tracing::init();

    let event_loop = EventLoop::new()?;

    println!(
        "This showcases the use of an input method engine (IME)
by emulating a text edit field.

Use CTRL+i to toggle IME support.
Use CTRL+p to cycle content purpose values.
Use CTRL+h to cycle content hint permutations.
"
    );

    // For alternative loop run options see `pump_events` and `run_on_demand` examples.
    event_loop.run_app(App {
        window: None,
        text: TextInputState {
            ime_enabled: true,
            contents: (String::new(), 0),
            purpose: ImePurpose::Normal,
            // The input field is not displayed at all in this demo.
            // This also makes it clear that the capability has been enabled.
            hint: ImeHint::HIDDEN_TEXT,
        },
        modifiers: ModifiersState::default(),
    })?;

    Ok(())
}
