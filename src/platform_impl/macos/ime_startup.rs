//! Detect Apple's first-key Korean IME fallback. Never compose or rewrite text.
#[derive(Debug, Default)]
pub(super) struct ImeStartup {
    source: String,
    attempted: bool,
    serial: u64,
    batch: Option<Batch>,
}

#[derive(Debug)]
struct Batch {
    expected: char,
    inserted: bool,
}

impl ImeStartup {
    pub fn begin(&mut self, source: &str, characters: &str, eligible: bool) -> u64 {
        self.serial = self.serial.wrapping_add(1);
        self.batch = None;
        if source != self.source {
            self.source = source.to_owned();
            self.attempted = false;
        }
        if eligible && !self.attempted && source.starts_with("com.apple.inputmethod.Korean.") {
            let mut chars = characters.chars();
            if let Some(expected @ '\u{3131}'..='\u{318E}') = chars.next() {
                if chars.next().is_none() {
                    self.batch = Some(Batch { expected, inserted: false });
                }
            }
        }
        self.serial
    }

    pub fn insert(&mut self, text: &str) {
        if let Some(batch) = &mut self.batch {
            let mut chars = text.chars();
            if !batch.inserted && chars.next() == Some(batch.expected) && chars.next().is_none() {
                batch.inserted = true;
            } else {
                self.batch = None;
            }
        }
    }

    // A preedit, command or unmark callback means AppKit already handled this batch.
    pub fn other_callback(&mut self) {
        self.batch = None;
    }

    pub fn finish(&mut self, serial: u64, same_context: bool) -> bool {
        // A nested keyDown or IME reset invalidates the outer interpretation.
        if serial != self.serial {
            return false;
        }
        let retry = same_context && self.batch.take().is_some_and(|batch| batch.inserted);
        self.batch = None;
        if retry {
            // Disarm BEFORE re-entering AppKit; callbacks must never cause recursive retries.
            self.attempted = true;
        }
        retry
    }

    pub fn reset(&mut self) {
        self.serial = self.serial.wrapping_add(1);
        self.attempted = false;
        self.batch = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const KO: &str = "com.apple.inputmethod.Korean.2SetKorean";
    fn fallback(s: &mut ImeStartup, source: &str, event: &str, text: &str, eligible: bool) -> bool {
        let serial = s.begin(source, event, eligible);
        s.insert(text);
        s.finish(serial, true)
    }
    #[test]
    fn cold_single_jamo_gets_one_retry_only() {
        let mut s = ImeStartup::default();
        assert!(fallback(&mut s, KO, "ㅎ", "ㅎ", true));
        assert!(!fallback(&mut s, KO, "ㅎ", "ㅎ", true));
    }
    #[test]
    fn language_change_and_ime_reset_start_new_attempts() {
        let mut s = ImeStartup::default();
        assert!(fallback(&mut s, KO, "ㅎ", "ㅎ", true));
        assert!(!fallback(&mut s, "com.apple.keylayout.ABC", "a", "a", true));
        assert!(fallback(&mut s, KO, "ㅎ", "ㅎ", true));
        s.reset();
        assert!(fallback(&mut s, KO, "ㅎ", "ㅎ", true));
    }
    #[test]
    fn preserves_ascii_paste_multi_char_other_layouts_and_shortcuts() {
        for (source, event, text, eligible) in [
            (KO, "a", "a", true),
            (KO, "ㅎ", "한", true),
            (KO, "ㅎ", "ㅎㅏ", true),
            (KO, "ㅎ", "ㄱ", true),
            ("com.apple.keylayout.ABC", "ㅎ", "ㅎ", true),
            ("com.apple.inputmethod.Kotoeri.Japanese", "ㅎ", "ㅎ", true),
            (KO, "ㅎ", "ㅎ", false),
            (KO, "", "ㅎ", true),
        ] {
            assert!(!fallback(&mut ImeStartup::default(), source, event, text, eligible));
        }
    }
    #[test]
    fn no_callback_is_not_a_fallback() {
        let mut s = ImeStartup::default();
        let id = s.begin(KO, "ㅎ", true);
        assert!(!s.finish(id, true));
    }
    #[test]
    fn multiple_inserts_or_any_other_native_callback_prevent_replay() {
        let mut s = ImeStartup::default();
        let id = s.begin(KO, "ㅎ", true);
        s.insert("ㅎ");
        s.insert("ㅎ");
        assert!(!s.finish(id, true));
        for before in [true, false] {
            let id = s.begin(KO, "ㅎ", true);
            if before {
                s.other_callback();
            }
            s.insert("ㅎ");
            if !before {
                s.other_callback();
            }
            assert!(!s.finish(id, true));
        }
    }
    #[test]
    fn nested_key_cannot_replay_outer_key() {
        let mut s = ImeStartup::default();
        let outer = s.begin(KO, "ㅎ", true);
        s.insert("ㅎ");
        let inner = s.begin(KO, "ㄱ", true);
        s.insert("ㄱ");
        assert!(s.finish(inner, true));
        assert!(!s.finish(outer, true));
    }
    #[test]
    fn focus_source_or_ime_change_during_callback_prevents_replay() {
        let mut s = ImeStartup::default();
        let id = s.begin(KO, "ㅎ", true);
        s.insert("ㅎ");
        assert!(!s.finish(id, false));
        let id = s.begin(KO, "ㅎ", true);
        s.insert("ㅎ");
        s.reset();
        assert!(!s.finish(id, true));
    }
}
