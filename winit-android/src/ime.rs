use android_activity::input::TextInputState;
use winit_core::event::Ime;

#[derive(Debug, Default)]
pub(crate) struct ImeState {
    committed: String,
    preedit: Option<(String, (usize, usize))>,
    pub(crate) enabled: bool,
    pub(crate) notifications: Vec<Ime>,
}

impl ImeState {
    pub(crate) fn enable(&mut self) {
        self.committed.clear();
        self.preedit = None;
        self.enabled = true;
        self.notifications.push(Ime::Enabled);
    }

    pub(crate) fn disable(&mut self) {
        if self.enabled {
            self.enabled = false;
            self.notifications.push(Ime::Disabled);
        }
    }

    pub(crate) fn update(&mut self, state: &TextInputState) -> Vec<Ime> {
        if !self.enabled {
            return Vec::new();
        }

        let mut committed = state.text.clone();
        let preedit = state.compose_region.as_ref().and_then(|span| {
            let start = utf8_offset(&state.text, span.start);
            let end = utf8_offset(&state.text, span.end);
            (start < end).then(|| {
                let cursor = |offset| utf8_offset(&state.text, offset).clamp(start, end) - start;
                let selection = (cursor(state.selection.start), cursor(state.selection.end));
                let text = committed.drain(start..end).collect();
                (text, selection)
            })
        });

        // GameActivity sends a snapshot of its editable buffer, including text
        // already committed to the application. Only send the changed suffix.
        let common = self
            .committed
            .chars()
            .zip(committed.chars())
            .take_while(|(old, new)| old == new)
            .map(|(ch, _)| ch.len_utf8())
            .sum::<usize>();
        let mut events = Vec::new();
        if self.committed.len() > common {
            events.push(Ime::DeleteSurrounding {
                before_bytes: self.committed.len() - common,
                after_bytes: 0,
            });
        }
        let did_commit = committed.len() > common;
        if did_commit {
            events.push(Ime::Commit(committed[common..].to_owned()));
        }
        if preedit != self.preedit || did_commit {
            match &preedit {
                Some((text, selection)) => {
                    events.push(Ime::Preedit(text.clone(), Some(*selection)));
                },
                None if self.preedit.is_some() && !did_commit => {
                    events.push(Ime::Preedit(String::new(), None));
                },
                None => (),
            }
        }
        self.committed = committed;
        self.preedit = preedit;
        events
    }
}

// Java reports offsets in UTF-16 code units. Round a split surrogate down to
// its scalar boundary and clamp positions past the end to the string length.
fn utf8_offset(text: &str, offset: usize) -> usize {
    let mut utf16 = 0;
    for (byte, ch) in text.char_indices() {
        utf16 += ch.len_utf16();
        if utf16 > offset {
            return byte;
        }
    }
    text.len()
}
