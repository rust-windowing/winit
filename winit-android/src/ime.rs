//! Translation of `GameTextInput` snapshots into [`Ime`] events.
//!
//! `GameTextInput` keeps its own copy of the text field and, after every change the input method
//! makes to it, reports the whole buffer: text, selection and composing region. Winit reports IME
//! input as edits relative to the selection instead, so [`ImeState`] remembers what the
//! application has been told so far and turns each new snapshot into the events that bring the
//! application to the same state.

use winit_core::event::Ime;

/// The text field as the application sees it, built from the IME events sent so far.
#[derive(Debug, Default)]
pub(crate) struct ImeState {
    /// The text of the field, without the preedit.
    text: String,
    /// Caret, in bytes into `text`. The preedit is shown here.
    cursor: usize,
    /// Other end of the selection, in bytes into `text`.
    anchor: usize,
    /// The preedit currently shown, empty when there is none.
    preedit: String,
    /// The selection within `preedit`, in bytes.
    preedit_cursor: Option<(usize, usize)>,
    /// Whether the application asked for the IME.
    enabled: bool,
    /// Whether the application was last told that the IME is enabled.
    reported: bool,
}

/// A snapshot of the `GameTextInput` buffer. Offsets are in UTF-16 code units, as Java counts
/// them.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Snapshot<'a> {
    pub text: &'a str,
    /// Selection anchor, the end that stays put when the selection is extended.
    pub anchor: usize,
    /// Selection end that moves, where the caret is drawn.
    pub cursor: usize,
    /// The composing region, if the input method is composing.
    pub compose: Option<(usize, usize)>,
}

/// Text and selection to load into `GameTextInput`, in UTF-16 code units.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Buffer {
    pub text: String,
    pub anchor: usize,
    pub cursor: usize,
    pub compose: Option<(usize, usize)>,
}

impl ImeState {
    /// Start reporting input. The field starts out empty, since disabling forgets it.
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Stop reporting input, forgetting the field.
    pub fn disable(&mut self) {
        *self = Self { enabled: false, reported: self.reported, ..Self::default() };
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// The [`Ime::Enabled`] or [`Ime::Disabled`] event the application has not received yet.
    pub fn take_status_change(&mut self) -> Option<Ime> {
        if self.enabled == self.reported {
            return None;
        }
        self.reported = self.enabled;
        Some(if self.enabled { Ime::Enabled } else { Ime::Disabled })
    }

    /// Take `text` with the selection from byte `anchor` to byte `cursor` as the content of the
    /// field, as the application describes it.
    ///
    /// Returns the buffer to load into `GameTextInput`, or `None` when it already holds this
    /// state, which is the case when the application echoes back an edit the input method just
    /// made. `GameTextInput` restarts the input connection whenever its buffer is replaced, and
    /// the keyboard then drops the word it is composing, so this must not reload needlessly.
    pub fn load(&mut self, text: &str, cursor: usize, anchor: usize) -> Option<Buffer> {
        if self.text == text && self.cursor == cursor && self.anchor == anchor {
            return None;
        }

        text.clone_into(&mut self.text);
        self.cursor = cursor;
        self.anchor = anchor;

        // Keep the preedit inside the new buffer so the composition can go on.
        let mut text = self.text.clone();
        text.insert_str(self.cursor, &self.preedit);
        let compose = (!self.preedit.is_empty()).then(|| {
            (
                utf16_offset(&text, self.cursor),
                utf16_offset(&text, self.cursor + self.preedit.len()),
            )
        });
        let (anchor, cursor) = if self.preedit.is_empty() {
            (utf16_offset(&text, self.anchor), utf16_offset(&text, self.cursor))
        } else {
            let end = utf16_offset(&text, self.cursor + self.preedit.len());
            (end, end)
        };

        Some(Buffer { text, anchor, cursor, compose })
    }

    /// Record a new snapshot of the `GameTextInput` buffer and return the events that turn the
    /// field the application has into the one in the snapshot, in the order they must be applied.
    pub fn update(&mut self, snapshot: Snapshot<'_>) -> Vec<Ime> {
        let Snapshot { text, anchor, cursor, compose } = snapshot;
        let anchor = byte_offset(text, anchor);
        let cursor = byte_offset(text, cursor);

        let compose = compose
            .map(|(start, end)| (byte_offset(text, start), byte_offset(text, end)))
            .map(|(start, end)| (start.min(end), start.max(end)))
            .filter(|(start, end)| start < end);

        // Split the snapshot into the text of the field and the preedit shown at the caret.
        let (new_text, preedit, preedit_cursor) = match compose {
            Some((start, end)) => {
                let new_text = [&text[..start], &text[end..]].concat();
                // The caret within the preedit, hidden when it is outside of it.
                let preedit_cursor = [anchor, cursor]
                    .iter()
                    .all(|offset| (start..=end).contains(offset))
                    .then(|| (anchor.min(cursor) - start, anchor.max(cursor) - start));
                (new_text, &text[start..end], preedit_cursor)
            },
            None => (text.to_owned(), "", None),
        };

        let mut events = Vec::new();
        let edited = self.text != new_text;
        if edited {
            let edit = self.edit(&new_text);
            if edit.before_bytes > 0 || edit.after_bytes > 0 {
                events.push(Ime::DeleteSurrounding {
                    before_bytes: edit.before_bytes,
                    after_bytes: edit.after_bytes,
                });
            }
            // An empty commit still replaces the selection.
            if !edit.commit.is_empty() || self.anchor != self.cursor {
                events.push(Ime::Commit(edit.commit.clone()));
            }

            let caret = edit.start + edit.commit.len();
            self.text = new_text;
            self.cursor = caret;
            self.anchor = caret;
        }

        // The application removes the preedit before deleting around the selection, and a commit
        // removes it on its own.
        let removed = edited || preedit.is_empty();
        if !self.preedit.is_empty() && removed && !matches!(events.first(), Some(Ime::Commit(_))) {
            events.insert(0, Ime::Preedit(String::new(), None));
        }

        let preedit_changed = self.preedit != preedit || self.preedit_cursor != preedit_cursor;
        if !preedit.is_empty() && (edited || preedit_changed) {
            events.push(Ime::Preedit(preedit.to_owned(), preedit_cursor));
        }
        preedit.clone_into(&mut self.preedit);
        self.preedit_cursor = preedit_cursor;

        events
    }

    /// The edit that turns the text of the field into `new`.
    ///
    /// The changed span lies between the common prefix and the common suffix of both texts. It is
    /// widened to cover the selection, since the edit is applied there: the selection is replaced
    /// by the committed text, which also carries any changed text between the selection and the
    /// span. A keyboard correcting the word behind the caret, for instance, deletes back to the
    /// start of the word and commits the corrected word.
    fn edit(&self, new: &str) -> Edit {
        let old = self.text.as_str();
        let selection_start = self.cursor.min(self.anchor).min(old.len());
        let selection_end = self.cursor.max(self.anchor).min(old.len());

        // Char-wise: byte-wise would split multi-byte UTF-8.
        let prefix: usize = old
            .chars()
            .zip(new.chars())
            .take_while(|(a, b)| a == b)
            .map(|(a, _)| a.len_utf8())
            .sum();
        let suffix: usize = old[prefix..]
            .chars()
            .rev()
            .zip(new[prefix..].chars().rev())
            .take_while(|(a, b)| a == b)
            .map(|(a, _)| a.len_utf8())
            .sum();

        let start = prefix.min(selection_start);
        let end = (old.len() - suffix).max(selection_end);
        // Past the changed span both texts match, so `old[end..]` is also the tail of `new`.
        let commit = new[start..new.len() - (old.len() - end)].to_owned();

        Edit {
            start,
            before_bytes: selection_start - start,
            after_bytes: end - selection_end,
            commit,
        }
    }
}

/// An edit relative to the selection of the field.
#[derive(Debug)]
struct Edit {
    /// Where the edit starts, in bytes into the text before it.
    start: usize,
    /// Bytes to delete before the selection.
    before_bytes: usize,
    /// Bytes to delete after the selection.
    after_bytes: usize,
    /// Text replacing the selection.
    commit: String,
}

/// UTF-16 offset of byte `byte` in `text`, clamped to it.
fn utf16_offset(text: &str, byte: usize) -> usize {
    text.char_indices().take_while(|(at, _)| *at < byte).map(|(_, ch)| ch.len_utf16()).sum()
}

/// Byte offset of the UTF-16 offset `units` in `text`, clamped to it.
fn byte_offset(text: &str, units: usize) -> usize {
    let mut count = 0;
    for (byte, ch) in text.char_indices() {
        if count >= units {
            return byte;
        }
        count += ch.len_utf16();
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(text: &str, caret: usize) -> Snapshot<'_> {
        let caret = text[..caret].encode_utf16().count();
        Snapshot { text, anchor: caret, cursor: caret, compose: None }
    }

    fn typed(state: &mut ImeState, text: &str) -> Vec<Ime> {
        state.update(snapshot(text, text.len()))
    }

    fn composing(state: &mut ImeState, text: &str, compose: (usize, usize)) -> Vec<Ime> {
        let caret = text.encode_utf16().count();
        state.update(Snapshot { text, anchor: caret, cursor: caret, compose: Some(compose) })
    }

    fn commit(text: &str) -> Ime {
        Ime::Commit(text.into())
    }

    fn delete(before_bytes: usize, after_bytes: usize) -> Ime {
        Ime::DeleteSurrounding { before_bytes, after_bytes }
    }

    fn preedit(text: &str, cursor: usize) -> Ime {
        Ime::Preedit(text.into(), Some((cursor, cursor)))
    }

    fn clear_preedit() -> Ime {
        Ime::Preedit(String::new(), None)
    }

    #[test]
    fn status_changes_are_reported_once() {
        let mut state = ImeState::default();
        assert_eq!(state.take_status_change(), None);
        state.enable();
        assert_eq!(state.take_status_change(), Some(Ime::Enabled));
        assert_eq!(state.take_status_change(), None);
        state.disable();
        state.enable();
        assert_eq!(state.take_status_change(), None, "back to enabled before the app noticed");
        state.disable();
        assert_eq!(state.take_status_change(), Some(Ime::Disabled));
    }

    #[test]
    fn typing_commits_the_new_text() {
        let mut state = ImeState::default();
        assert_eq!(typed(&mut state, "ho"), [commit("ho")]);
        assert_eq!(typed(&mut state, "hol"), [commit("l")]);
        assert_eq!(typed(&mut state, "ho"), [delete(1, 0)]);
    }

    #[test]
    fn a_correction_replaces_the_word_behind_the_caret() {
        let mut state = ImeState::default();
        typed(&mut state, "helo ");
        assert_eq!(typed(&mut state, "hello "), [delete(2, 0), commit("lo ")]);
    }

    #[test]
    fn typing_over_a_selection_replaces_only_the_selection() {
        let mut state = ImeState::default();
        assert!(state.load("hello world", 11, 6).is_some());
        assert_eq!(typed(&mut state, "hello x"), [commit("x")]);
        assert_eq!(state.text, "hello x");
    }

    #[test]
    fn deleting_a_selection_commits_nothing_in_its_place() {
        let mut state = ImeState::default();
        state.load("hello world", 6, 11);
        assert_eq!(typed(&mut state, "hello "), [commit("")]);
    }

    #[test]
    fn composition_is_reported_as_preedit() {
        let mut state = ImeState::default();
        assert_eq!(composing(&mut state, "h", (0, 1)), [preedit("h", 1)]);
        assert_eq!(composing(&mut state, "he", (0, 2)), [preedit("he", 2)]);
        // The keyboard commits its correction of the composing word.
        assert_eq!(typed(&mut state, "hello "), [commit("hello ")]);
        assert_eq!(state.preedit, "");
    }

    #[test]
    fn deleting_the_whole_preedit_clears_it() {
        let mut state = ImeState::default();
        typed(&mut state, "a ");
        composing(&mut state, "a b", (2, 3));
        assert_eq!(typed(&mut state, "a "), [clear_preedit()]);
    }

    #[test]
    fn editing_around_a_preedit_clears_it_first() {
        let mut state = ImeState::default();
        typed(&mut state, "ab ");
        composing(&mut state, "ab c", (3, 4));
        // The keyboard deletes the space before the composing word.
        assert_eq!(composing(&mut state, "abc", (2, 3)), [
            clear_preedit(),
            delete(1, 0),
            preedit("c", 1)
        ]);
    }

    #[test]
    fn composing_an_existing_word_moves_it_into_the_preedit() {
        let mut state = ImeState::default();
        typed(&mut state, "hello ");
        assert_eq!(typed(&mut state, "hello"), [delete(1, 0)]);
        // The keyboard picks the word behind the caret up again to correct it.
        assert_eq!(composing(&mut state, "hello", (0, 5)), [delete(5, 0), preedit("hello", 5)]);
        assert_eq!(state.text, "");
    }

    #[test]
    fn a_preedit_with_the_caret_outside_hides_the_preedit_caret() {
        let mut state = ImeState::default();
        typed(&mut state, "ab ");
        let events =
            state.update(Snapshot { text: "ab cd", anchor: 0, cursor: 0, compose: Some((3, 5)) });
        assert_eq!(events, [Ime::Preedit("cd".into(), None)]);
    }

    #[test]
    fn caret_moves_alone_send_nothing() {
        let mut state = ImeState::default();
        typed(&mut state, "hello world");
        assert!(state.update(snapshot("hello world", 5)).is_empty());
        // The application still has the caret at the end: the edit is widened to reach it.
        assert_eq!(state.update(snapshot("hellox world", 6)), [delete(6, 0), commit("x world")]);
        assert_eq!(state.text, "hellox world");
    }

    #[test]
    fn loaded_text_is_the_new_baseline() {
        let mut state = ImeState::default();
        let buffer = state.load("día uno", 5, 5);
        assert_eq!(
            buffer,
            Some(Buffer { text: "día uno".into(), anchor: 4, cursor: 4, compose: None })
        );
        assert_eq!(state.load("día uno", 5, 5), None, "already loaded");
        assert_eq!(state.update(snapshot("día xuno", 6)), [commit("x")]);
        // The application echoes the edit back: nothing to load.
        assert_eq!(state.load("día xuno", 6, 6), None);
    }

    #[test]
    fn loading_keeps_the_preedit_composing() {
        let mut state = ImeState::default();
        composing(&mut state, "ab", (0, 2));
        let buffer = state.load("x", 1, 1);
        assert_eq!(
            buffer,
            Some(Buffer { text: "xab".into(), anchor: 3, cursor: 3, compose: Some((1, 3)) })
        );
    }

    #[test]
    fn multi_byte_text_is_split_on_char_boundaries() {
        let mut state = ImeState::default();
        typed(&mut state, "año");
        assert_eq!(typed(&mut state, "añ"), [delete(1, 0)]);
        assert_eq!(typed(&mut state, "a😀"), [delete(2, 0), commit("😀")]);
    }
}
