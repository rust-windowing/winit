//! Showcases the use of an input method engine (IME)
//! by emulating a text edit field.
//!
//! Use CTRL+i to toggle IME support.
//! Use CTRL+p to cycle content purpose values.
//! Use CTRL+h to cycle content hint permutations.

use std::cmp;
use std::error::Error;

use dpi::{LogicalPosition, PhysicalSize};
use tracing::{error, info};
use winit::application::ApplicationHandler;
use winit::event::{Ime, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
#[cfg(web_platform)]
use winit::platform::web::WindowAttributesWeb;
use winit::window::{
    ImeCapabilities, ImeEnableRequest, ImeHint, ImePurpose, ImeRequest, ImeRequestData,
    ImeSurroundingText, Window, WindowAttributes, WindowId,
};

#[path = "util/fill.rs"]
mod fill;
#[path = "util/tracing.rs"]
mod tracing_init;

const IME_CURSOR_SIZE: PhysicalSize<u32> = PhysicalSize::new(20, 20);

#[derive(Debug)]
struct App {
    window: Option<Box<dyn Window>>,
    input_state: TextInputState,
    modifiers: ModifiersState,
}

/// State of the undisplayed text input field.
#[derive(Debug)]
struct TextInputState {
    ime_enabled: bool,
    /// The contents of the emulated text field for IME purposes (not displayed).
    /// (text, cursor position in bytes).
    contents: String,
    /// The purpose of the contents the emulated text field expects
    purpose: ImePurpose,
    /// The behaviour hints for the IME regarding the emulated text field
    hint: ImeHint,
}

impl TextInputState {
    fn text_and_cursor(&self) -> (&str, usize) {
        (&self.contents, self.contents.len())
    }

    fn append_text(&mut self, text: &str) {
        self.contents.push_str(text);
    }

    fn set_text(&mut self, text: String) {
        self.contents = text;
    }

    fn pop(&mut self) {
        self.contents.pop();
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
                .with_hint_and_purpose()
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
                info!("Close was requested; stopping");
                self.window = None;
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
    fn handle_key_pressed(&mut self, event: winit::event::KeyEvent, mods: ModifiersState) {
        match event.key_without_modifiers.as_ref() {
            Key::Character("i") if mods == ModifiersState::CONTROL => self.toggle_ime(),
            Key::Character("p") if mods == ModifiersState::CONTROL => {
                self.input_state.purpose = match self.input_state.purpose {
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
                if self.input_state.ime_enabled {
                    self.window()
                        .request_ime_update(ImeRequest::Update(self.get_ime_update()))
                        .unwrap();
                }
                info!("text input purpose now {:?}", self.input_state.purpose);
            },
            Key::Character("h") if mods == ModifiersState::CONTROL => {
                let bump = |hint: ImeHint| {
                    if hint.is_all() {
                        ImeHint::NONE
                    } else {
                        // Go through all integers. We'll skip invalid ones
                        ImeHint::from_bits_retain(hint.bits().wrapping_add(1))
                    }
                };
                let mut new_hint = bump(self.input_state.hint);

                while !ImeHint::all().contains(new_hint) {
                    new_hint = bump(new_hint);
                }

                self.input_state.hint = new_hint;

                if self.input_state.ime_enabled {
                    self.window()
                        .request_ime_update(ImeRequest::Update(self.get_ime_update()))
                        .unwrap();
                }
                info!("text input IME hint now {:?}", self.input_state.hint);
            },
            Key::Named(NamedKey::Backspace) => {
                self.input_state.pop();
                self.print_input_state();
            },
            _ => {
                if let Some(text) = event.text {
                    self.input_state.append_text(&text);
                    if self.input_state.ime_enabled {
                        self.window()
                            .request_ime_update(ImeRequest::Update(self.get_ime_update()))
                            .unwrap();
                    }
                    self.print_input_state();
                }
            },
        }
    }

    fn handle_ime_event(&mut self, event: Ime) {
        let window = self.window.as_ref().expect("IME request without a window");
        match event {
            Ime::Enabled => info!("IME enabled for Window={:?}", window.id()),
            Ime::Preedit(text, caret_pos) => info!("Preedit: {text}, with caret at {caret_pos:?}"),
            Ime::Commit(text) => {
                self.input_state.append_text(&text);
                let request_data = self.get_ime_update();
                window.request_ime_update(ImeRequest::Update(request_data)).unwrap();
                self.print_input_state();
            },
            Ime::DeleteSurrounding { before_bytes, after_bytes } => {
                let (text, cursor) = &self.input_state.text_and_cursor();

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
                    self.input_state.set_text(new_text);
                    info!("IME deleted bytes: {before_bytes}, {after_bytes}");
                    self.print_input_state();
                } else {
                    error!("Buggy IME tried to delete with indices not on char boundary.");
                }
            },
            Ime::Disabled => info!("IME disabled for Window={:?}", window.id()),
        }
    }

    fn toggle_ime(&mut self) {
        let enable = !self.input_state.ime_enabled;

        if enable {
            let enable_request = ImeEnableRequest::new(
                ImeCapabilities::new()
                    .with_hint_and_purpose()
                    .with_cursor_area()
                    .with_surrounding_text(),
                self.get_ime_update(),
            )
            .unwrap();
            self.window().request_ime_update(ImeRequest::Enable(enable_request)).unwrap();
        } else {
            self.window().disable_ime();
        };

        self.input_state.ime_enabled = enable;

        info!("IME enabled now {}", self.input_state.ime_enabled);
    }

    fn get_ime_update(&self) -> ImeRequestData {
        let text = &self.input_state.contents;
        let cursor = text.len();
        // A rudimentary text field emulation: the caret moves right by a constant amount for each
        // code point.

        let text_before_caret = if text.is_char_boundary(cursor) { &text[..cursor] } else { "" };
        let chars_before_caret = text_before_caret.chars().count();
        let cursor_pos = LogicalPosition::<u32> { x: 10 * chars_before_caret as u32, y: 0 };

        // Limit text field size
        const MAX_BYTES: usize = ImeSurroundingText::MAX_TEXT_BYTES;
        let minimal_offset = cursor / MAX_BYTES * MAX_BYTES;
        let first_char_boundary =
            (minimal_offset..cursor).find(|off| text.is_char_boundary(*off)).unwrap_or(cursor);
        let last_char_boundary = (cursor..(first_char_boundary + MAX_BYTES))
            .rev()
            .find(|off| text.is_char_boundary(*off))
            .unwrap_or(cursor);
        let surrounding_text = &text[first_char_boundary..last_char_boundary];
        let relative_cursor = cursor - first_char_boundary;
        let surrounding_text =
            ImeSurroundingText::new(surrounding_text.into(), relative_cursor, relative_cursor)
                .expect("Bug in example: bad byte calculations");

        ImeRequestData::default()
            .with_hint_and_purpose(self.input_state.hint, self.input_state.purpose)
            .with_cursor_area(cursor_pos.into(), IME_CURSOR_SIZE.into())
            .with_surrounding_text(surrounding_text)
    }

    fn print_input_state(&self) {
        let (text, cursor) = &self.input_state.text_and_cursor();
        // Representing a selection with the cursor and anchor as ends is not
        // supported yet. Using the same position for anchor to mark no
        // selection.
        info!("{}", preedit_with_cursor(text, *cursor, *cursor));
    }

    fn window(&self) -> &dyn Window {
        self.window.as_ref().unwrap().as_ref()
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

    tracing_init::init();

    let event_loop = EventLoop::new()?;

    println!(
        r#"This showcases the use of an input method engine (IME) by emulating a text edit field.
Use CTRL+i to toggle IME support.
Use CTRL+p to cycle content purpose values.
Use CTRL+h to cycle content hint permutations.
"#
    );

    let app = App {
        window: None,
        input_state: TextInputState {
            ime_enabled: true,
            contents: String::new(),
            purpose: ImePurpose::Normal,
            // While we don't show text and thus we use ImeHint::HIDDEN
            // it may cause the IME to not do layout switch, etc at all.
            hint: ImeHint::NONE,
        },
        modifiers: ModifiersState::default(),
    };

    // For alternative loop run options see `pump_events` and `run_on_demand` examples.
    event_loop.run_app(app)?;

    Ok(())
}
