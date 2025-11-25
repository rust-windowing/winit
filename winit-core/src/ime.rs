use std::fmt;

use bitflags::bitflags;
use dpi::{Position, Size};

/// Generic IME purposes for use in [`Window::set_ime_purpose`].
///
/// The purpose should reflect the kind of data to be entered.
/// The purpose may improve UX by optimizing the IME for the specific use case,
/// for example showing relevant characters and hiding unneeded ones,
/// or changing the icon of the confirmation button,
/// if winit can express the purpose to the platform and the platform reacts accordingly.
///
/// ## Platform-specific
///
/// - **iOS / Android / Web / Windows / X11 / macOS / Orbital:** Unsupported.
#[non_exhaustive]
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ImePurpose {
    /// No special purpose for the IME (default).
    #[default]
    Normal,
    /// The IME is used for password input.
    /// The IME will treat the contents as sensitive.
    Password,
    /// The IME is used to input into a terminal.
    ///
    /// For example, that could alter OSK on Wayland to show extra buttons.
    Terminal,
    /// Number (including decimal separator and sign)
    Number,
    /// Phone number
    Phone,
    /// URL
    Url,
    /// Email address
    Email,
    /// Password composed only of digits (treated as sensitive data)
    Pin,
    /// Date
    Date,
    /// Time
    Time,
    /// Date and time
    DateTime,
}

bitflags! {
    /// IME hints
    ///
    /// The hint should reflect the desired behaviour of the IME
    /// while entering text.
    /// The purpose may improve UX by optimizing the IME for the specific use case,
    /// beyond just the general data type specified in `ImePurpose`.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android / Web / Windows / X11 / macOS / Orbital:** Unsupported.
    #[non_exhaustive]
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct ImeHint: u32 {
        /// No special behaviour.
        const NONE = 0;
        /// Suggest word completions.
        const COMPLETION = 0x1;
        /// Suggest word corrections.
        const SPELLCHECK = 0x2;
        /// Switch to uppercase letters at the start of a sentence.
        const AUTO_CAPITALIZATION = 0x4;
        /// Prefer lowercase letters.
        const LOWERCASE = 0x8;
        /// Prefer uppercase letters.
        const UPPERCASE = 0x10;
        /// Prefer casing for titles and headings (can be language dependent).
        const TITLECASE = 0x20;
        /// Characters should be hidden.
        ///
        /// This may prevent e.g. layout switching with some IMEs, unless hint is disabled.
        const HIDDEN_TEXT = 0x40;
        /// Typed text should not be stored.
        const SENSITIVE_DATA = 0x80;
        /// Just Latin characters should be entered.
        const LATIN = 0x100;
        /// The text input is multiline.
        const MULTILINE = 0x200;
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum ImeSurroundingTextError {
    /// Text exceeds 4000 bytes
    TextTooLong,
    /// Cursor not on a code point boundary, or past the end of text.
    CursorBadPosition,
    /// Anchor not on a code point boundary, or past the end of text.
    AnchorBadPosition,
}

impl fmt::Display for ImeSurroundingTextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImeSurroundingTextError::TextTooLong => write!(f, "text exceeds maximum length"),
            ImeSurroundingTextError::CursorBadPosition => {
                write!(f, "cursor is not at a valid text index")
            },
            ImeSurroundingTextError::AnchorBadPosition => {
                write!(f, "anchor is not at a valid text index")
            },
        }
    }
}

impl std::error::Error for ImeSurroundingTextError {}

/// Defines the text surrounding the caret
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ImeSurroundingText {
    /// An excerpt of the text present in the text input field, excluding preedit.
    text: String,
    /// The position of the caret, in bytes from the beginning of the string
    cursor: usize,
    /// The position of the other end of selection, in bytes.
    /// With no selection, it should be the same as the cursor.
    anchor: usize,
}

impl ImeSurroundingText {
    /// The maximum size of the text excerpt.
    pub const MAX_TEXT_BYTES: usize = 4000;
    /// Defines the text surrounding the cursor and the selection within it.
    ///
    /// `text`: An excerpt of the text present in the text input field, excluding preedit.
    /// It must be limited to 4000 bytes due to backend constraints.
    /// `cursor`: The position of the caret, in bytes from the beginning of the string.
    /// `anchor: The position of the other end of selection, in bytes.
    /// With no selection, it should be the same as the cursor.
    ///
    /// This may fail if the byte indices don't fall on code point boundaries,
    /// or if the text is too long.
    ///
    /// ## Examples:
    ///
    /// A text field containing `foo|bar` where `|` denotes the caret would correspond to a value
    /// obtained by:
    ///
    /// ```
    /// # use winit_core::window::ImeSurroundingText;
    /// let s = ImeSurroundingText::new("foobar".into(), 3, 3).unwrap();
    /// ```
    ///
    /// Because preedit is excluded from the text string, a text field containing `foo[baz|]bar`
    /// where `|` denotes the caret and [baz|] is the preedit would be created in exactly the same
    /// way.
    pub fn new(
        text: String,
        cursor: usize,
        anchor: usize,
    ) -> Result<Self, ImeSurroundingTextError> {
        let text = if text.len() < 4000 {
            text
        } else {
            return Err(ImeSurroundingTextError::TextTooLong);
        };

        let cursor = if text.is_char_boundary(cursor) && cursor <= text.len() {
            cursor
        } else {
            return Err(ImeSurroundingTextError::CursorBadPosition);
        };

        let anchor = if text.is_char_boundary(anchor) && anchor <= text.len() {
            anchor
        } else {
            return Err(ImeSurroundingTextError::AnchorBadPosition);
        };

        Ok(Self { text, cursor, anchor })
    }

    /// Consumes the object, releasing the text string only.
    /// Use this call in the backend to avoid an extra clone when submitting the surrounding text.
    pub fn into_text(self) -> String {
        self.text
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn anchor(&self) -> usize {
        self.anchor
    }
}

/// Request to send to IME.
#[derive(Debug, PartialEq, Clone)]
pub enum ImeRequest {
    /// Enable the IME with the [`ImeCapabilities`] and [`ImeRequestData`] as initial state. When
    /// the [`ImeRequestData`] is **not** matching capabilities fully, the default values will be
    /// used instead.
    ///
    /// **Requesting to update data matching not enabled capabilities will result in update
    /// being ignored.** The winit backend in such cases is recommended to log a warning. This
    /// applies to both [`ImeRequest::Enable`] and [`ImeRequest::Update`]. For details on
    /// capabilities refer to [`ImeCapabilities`].
    ///
    /// To update the [`ImeCapabilities`], the IME must be disabled and then re-enabled.
    Enable(ImeEnableRequest),
    /// Update the state of already enabled IME. Issuing this request before [`ImeRequest::Enable`]
    /// will result in error.
    Update(ImeRequestData),
}

/// Initial IME request.
#[derive(Debug, Clone, PartialEq)]
pub struct ImeEnableRequest {
    capabilities: ImeCapabilities,
    request_data: ImeRequestData,
}

impl ImeEnableRequest {
    /// Create request for the [`ImeRequest::Enable`]
    ///
    /// This will return [`None`] if some capability was requested but its initial value was not
    /// set by the user or value was set by the user, but capability not requested.
    pub fn new(capabilities: ImeCapabilities, request_data: ImeRequestData) -> Option<Self> {
        if capabilities.cursor_area() ^ request_data.cursor_area.is_some() {
            return None;
        }

        if capabilities.hint_and_purpose() ^ request_data.hint_and_purpose.is_some() {
            return None;
        }

        if capabilities.surrounding_text() ^ request_data.surrounding_text.is_some() {
            return None;
        }
        Some(Self { capabilities, request_data })
    }

    /// [`ImeCapabilities`] to enable.
    pub const fn capabilities(&self) -> &ImeCapabilities {
        &self.capabilities
    }

    /// Request data attached to request.
    pub const fn request_data(&self) -> &ImeRequestData {
        &self.request_data
    }

    /// Destruct [`ImeEnableRequest`]  into its raw parts.
    pub fn into_raw(self) -> (ImeCapabilities, ImeRequestData) {
        (self.capabilities, self.request_data)
    }
}

/// IME capabilities supported by client.
///
/// For example, if the client doesn't support [`ImeCapabilities::cursor_area()`], then not enabling
/// it will make IME hide the popup window instead of placing it arbitrary over the
/// client's window surface.
///
/// When the capability is not enabled or not supported by the IME, trying to update its'
/// corresponding data with [`ImeRequest`] will be ignored.
///
/// New capabilities may be added to this struct in the future.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ImeCapabilities(ImeCapabilitiesFlags);

impl ImeCapabilities {
    /// Returns a new empty set of capabilities.
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks `hint and purpose` as supported.
    ///
    /// For more details see [`ImeRequestData::with_hint_and_purpose`].
    pub const fn with_hint_and_purpose(self) -> Self {
        Self(self.0.union(ImeCapabilitiesFlags::HINT_AND_PURPOSE))
    }

    /// Marks `hint and purpose` as unsupported.
    ///
    /// For more details see [`ImeRequestData::with_hint_and_purpose`].
    pub const fn without_hint_and_purpose(self) -> Self {
        Self(self.0.difference(ImeCapabilitiesFlags::HINT_AND_PURPOSE))
    }

    /// Returns `true` if `hint and purpose` is supported.
    pub const fn hint_and_purpose(&self) -> bool {
        self.0.contains(ImeCapabilitiesFlags::HINT_AND_PURPOSE)
    }

    /// Marks `cursor_area` as supported.
    ///
    /// For more details see [`ImeRequestData::with_cursor_area`].
    pub const fn with_cursor_area(self) -> Self {
        Self(self.0.union(ImeCapabilitiesFlags::CURSOR_AREA))
    }

    /// Marks `cursor_area` as unsupported.
    ///
    /// For more details see [`ImeRequestData::with_cursor_area`].
    pub const fn without_cursor_area(self) -> Self {
        Self(self.0.difference(ImeCapabilitiesFlags::CURSOR_AREA))
    }

    /// Returns `true` if `cursor_area` is supported.
    pub const fn cursor_area(&self) -> bool {
        self.0.contains(ImeCapabilitiesFlags::CURSOR_AREA)
    }

    /// Marks `surrounding_text` as supported.
    ///
    /// For more details see [`ImeRequestData::with_surrounding_text`].
    pub const fn with_surrounding_text(self) -> Self {
        Self(self.0.union(ImeCapabilitiesFlags::SURROUNDING_TEXT))
    }

    /// Marks `surrounding_text` as unsupported.
    ///
    /// For more details see [`ImeRequestData::with_surrounding_text`].
    pub const fn without_surrounding_text(self) -> Self {
        Self(self.0.difference(ImeCapabilitiesFlags::SURROUNDING_TEXT))
    }

    /// Returns `true` if `surrounding_text` is supported.
    pub const fn surrounding_text(&self) -> bool {
        self.0.contains(ImeCapabilitiesFlags::SURROUNDING_TEXT)
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub(crate) struct ImeCapabilitiesFlags : u8 {
        /// Client supports setting IME hint and purpose.
        const HINT_AND_PURPOSE = 1 << 0;
        /// Client supports reporting cursor area for IME popup to
        /// appear.
        const CURSOR_AREA = 1 << 1;
        /// Client supports reporting the text around the caret
        const SURROUNDING_TEXT = 1 << 2;
    }
}

/// The [`ImeRequest`] data to communicate to system's IME.
///
/// This applies multiple IME state properties at once.
/// Fields set to `None` are not updated and the previously sent
/// value is reused.
#[non_exhaustive]
#[derive(Debug, PartialEq, Clone, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ImeRequestData {
    /// Text input hint and purpose.
    ///
    /// To support updating it, enable [`ImeCapabilities::hint_and_purpose()`].
    pub hint_and_purpose: Option<(ImeHint, ImePurpose)>,
    /// The IME cursor area which should not be covered by the input method popup.
    ///
    /// To support updating it, enable [`ImeCapabilities::cursor_area()`].
    pub cursor_area: Option<(Position, Size)>,
    /// The text surrounding the caret
    ///
    /// To support updating it, enable [`ImeCapabilities::surrounding_text()`].
    pub surrounding_text: Option<ImeSurroundingText>,
}

impl ImeRequestData {
    /// Sets the hint and purpose of the current text input content.
    pub fn with_hint_and_purpose(self, hint: ImeHint, purpose: ImePurpose) -> Self {
        Self { hint_and_purpose: Some((hint, purpose)), ..self }
    }

    /// Sets the IME cursor editing area.
    ///
    /// The `position` is the top left corner of that area
    /// in surface coordinates and `size` is the size of this area starting from the position. An
    /// example of such area could be a input field in the UI or line in the editor.
    ///
    /// The windowing system could place a candidate box close to that area, but try to not obscure
    /// the specified area, so the user input to it stays visible.
    ///
    /// The candidate box is the window / popup / overlay that allows you to select the desired
    /// characters. The look of this box may differ between input devices, even on the same
    /// platform.
    ///
    /// (Apple's official term is "candidate window", see their [chinese] and [japanese] guides).
    ///
    /// ## Example
    ///
    /// ```no_run
    /// # use dpi::{LogicalPosition, PhysicalPosition, LogicalSize, PhysicalSize};
    /// # use winit_core::window::ImeRequestData;
    /// # fn scope(ime_request_data: ImeRequestData) {
    /// // Specify the position in logical dimensions like this:
    /// let ime_request_data = ime_request_data.with_cursor_area(
    ///     LogicalPosition::new(400.0, 200.0).into(),
    ///     LogicalSize::new(100, 100).into(),
    /// );
    ///
    /// // Or specify the position in physical dimensions like this:
    /// let ime_request_data = ime_request_data.with_cursor_area(
    ///     PhysicalPosition::new(400, 200).into(),
    ///     PhysicalSize::new(100, 100).into(),
    /// );
    /// # }
    /// ```
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android / Web / Orbital:** Unsupported.
    ///
    /// [chinese]: https://support.apple.com/guide/chinese-input-method/use-the-candidate-window-cim12992/104/mac/12.0
    /// [japanese]: https://support.apple.com/guide/japanese-input-method/use-the-candidate-window-jpim10262/6.3/mac/12.0
    pub fn with_cursor_area(self, position: Position, size: Size) -> Self {
        Self { cursor_area: Some((position, size)), ..self }
    }

    /// Describes the text surrounding the caret.
    ///
    /// The IME can then continue providing suggestions for the continuation of the existing text,
    /// as well as can erase text more accurately, for example glyphs composed of multiple code
    /// points.
    pub fn with_surrounding_text(self, surrounding_text: ImeSurroundingText) -> Self {
        Self { surrounding_text: Some(surrounding_text), ..self }
    }
}

/// Error from sending request to IME with
/// [`Window::request_ime_update`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ImeRequestError {
    /// IME is not yet enabled.
    NotEnabled,
    /// IME is already enabled.
    AlreadyEnabled,
    /// Not supported.
    NotSupported,
}

impl fmt::Display for ImeRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImeRequestError::NotEnabled => write!(f, "ime is not enabled."),
            ImeRequestError::AlreadyEnabled => write!(f, "ime is already enabled."),
            ImeRequestError::NotSupported => write!(f, "ime is not supported."),
        }
    }
}

impl std::error::Error for ImeRequestError {}

#[cfg(test)]
mod tests {
    use dpi::{LogicalPosition, LogicalSize, Position, Size};

    use super::{
        ImeCapabilities, ImeEnableRequest, ImeHint, ImePurpose, ImeRequestData, ImeSurroundingText,
        ImeSurroundingTextError,
    };

    #[test]
    fn ime_initial_request_caps_match() {
        let position: Position = LogicalPosition::new(0, 0).into();
        let size: Size = LogicalSize::new(0, 0).into();

        assert!(
            ImeEnableRequest::new(
                ImeCapabilities::new().with_cursor_area(),
                ImeRequestData::default()
            )
            .is_none()
        );
        assert!(
            ImeEnableRequest::new(
                ImeCapabilities::new().with_hint_and_purpose(),
                ImeRequestData::default()
            )
            .is_none()
        );

        assert!(
            ImeEnableRequest::new(
                ImeCapabilities::new().with_cursor_area(),
                ImeRequestData::default().with_hint_and_purpose(ImeHint::NONE, ImePurpose::Normal)
            )
            .is_none()
        );

        assert!(
            ImeEnableRequest::new(
                ImeCapabilities::new(),
                ImeRequestData::default()
                    .with_hint_and_purpose(ImeHint::NONE, ImePurpose::Normal)
                    .with_cursor_area(position, size)
            )
            .is_none()
        );

        assert!(
            ImeEnableRequest::new(
                ImeCapabilities::new().with_cursor_area(),
                ImeRequestData::default()
                    .with_hint_and_purpose(ImeHint::NONE, ImePurpose::Normal)
                    .with_cursor_area(position, size)
            )
            .is_none()
        );

        assert!(
            ImeEnableRequest::new(
                ImeCapabilities::new().with_cursor_area(),
                ImeRequestData::default().with_cursor_area(position, size)
            )
            .is_some()
        );

        assert!(
            ImeEnableRequest::new(
                ImeCapabilities::new().with_hint_and_purpose().with_cursor_area(),
                ImeRequestData::default()
                    .with_hint_and_purpose(ImeHint::NONE, ImePurpose::Normal)
                    .with_cursor_area(position, size)
            )
            .is_some()
        );

        let text: &[u8] = ['a' as u8; 8000].as_slice();
        let text = std::str::from_utf8(text).unwrap();
        assert_eq!(
            ImeSurroundingText::new(text.into(), 0, 0),
            Err(ImeSurroundingTextError::TextTooLong),
        );

        assert_eq!(
            ImeSurroundingText::new("short".into(), 110, 0),
            Err(ImeSurroundingTextError::CursorBadPosition),
        );

        assert_eq!(
            ImeSurroundingText::new("граница".into(), 1, 0),
            Err(ImeSurroundingTextError::CursorBadPosition),
        );
    }
}
