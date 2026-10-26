//! Text encoding helpers shared by platform backends.

/// Convert a UTF-16 code unit offset into the corresponding UTF-8 byte offset within `s`.
///
/// IMEs are not required to send well-formed offsets, so this is defensive: an offset that
/// would split a surrogate pair is snapped down to the start of that character, and an
/// out-of-bounds offset is clamped to the end of the string (e.g. native Pinyin sends
/// out-of-bounds indices, see <https://github.com/alacritty/alacritty/issues/8791>).
///
/// The mapping is monotone non-decreasing, so applying it to the location and end of a
/// range (where `location <= end`) can never produce an inverted byte range.
pub fn utf16_to_utf8_offset(s: &str, utf16_offset: usize) -> usize {
    let mut utf16_pos = 0;
    for (utf8_pos, ch) in s.char_indices() {
        if utf16_pos >= utf16_offset {
            return utf8_pos;
        }
        utf16_pos += ch.len_utf16();
        // The target offset lands strictly inside this character's UTF-16 representation,
        // i.e. it splits a surrogate pair: snap down to the character boundary.
        if utf16_pos > utf16_offset {
            return utf8_pos;
        }
    }
    s.len()
}

#[cfg(test)]
mod tests {
    use super::utf16_to_utf8_offset;

    /// Apply the UTF-16 -> UTF-8 conversion to both ends of a `selectedRange {loc, len}`,
    /// mirroring what `set_marked_text` does for the emitted `Ime::Preedit` cursor range.
    fn convert(s: &str, loc: usize, len: usize) -> (usize, usize) {
        (utf16_to_utf8_offset(s, loc), utf16_to_utf8_offset(s, loc + len))
    }

    #[test]
    fn mid_surrogate_offset_snaps_down() {
        // "😀a": 😀 is one char = 2 UTF-16 units = 4 UTF-8 bytes; offset 1 is mid-pair.
        assert_eq!(utf16_to_utf8_offset("\u{1F600}a", 1), 0);
        // Offset 2 is the boundary just after the pair.
        assert_eq!(utf16_to_utf8_offset("\u{1F600}a", 2), 4);
    }

    #[test]
    fn no_longer_inverted() {
        // "a😀b" with selectedRange {1,1}: previously emitted (1, 0) -- lower > upper, a
        // slice-panic vector. The boundary-snapping conversion keeps lower <= upper.
        assert_eq!(convert("a\u{1F600}b", 1, 1), (1, 1));
    }

    #[test]
    fn prefix_preserved_on_mid_pair_collapse() {
        // "a😀b" with selectedRange {2,0}: previously collapsed to (0, 0), discarding the
        // valid "a" prefix; now snaps to the char boundary after "a".
        assert_eq!(convert("a\u{1F600}b", 2, 0), (1, 1));
    }

    #[test]
    fn out_of_bounds_clamps_to_len() {
        // Subsumes the #4494 `.min(len)` clamp: an out-of-bounds offset maps to the string
        // length instead of triggering an NSRangeException.
        assert_eq!(convert("\u{1F600}a", 99, 0), (5, 5));
    }

    #[test]
    fn well_formed_inputs_are_identity() {
        // The common case (well-formed boundary indices) must be byte-for-byte unchanged.
        assert_eq!(convert("a\u{1F600}b", 3, 0), (5, 5));
        assert_eq!(convert("a\u{1F600}b", 4, 0), (6, 6));
        // BMP multi-byte (Japanese): each char is 1 UTF-16 unit and 3 UTF-8 bytes.
        assert_eq!(convert("\u{3053}\u{3093}", 1, 1), (3, 6));
    }

    #[test]
    fn monotone_non_decreasing() {
        // Sweep every UTF-16 offset (including out-of-bounds) over a string mixing BMP and
        // non-BMP characters and assert the conversion never goes backwards, which is what
        // guarantees `lower <= upper` for any `NSRange`.
        let s = "a\u{1F600}b\u{3053}\u{1F4A9}c";
        let mut prev = 0;
        for off in 0..=20 {
            let cur = utf16_to_utf8_offset(s, off);
            assert!(cur >= prev, "non-monotone at offset {off}: {cur} < {prev}");
            assert!(cur <= s.len(), "offset {off} mapped past end: {cur} > {}", s.len());
            prev = cur;
        }
    }

    #[test]
    fn cursor_offsets_use_utf16_units() {
        for (text, units, expected) in [
            // A valid cursor after the emoji must not advance past the following character.
            ("a😀b", 3, 5),
            ("😀😀", 2, 4),
            // Empty, ASCII and BMP text retain their existing offsets.
            ("", 0, 0),
            ("", 1, 0),
            ("ab", 0, 0),
            ("ab", 1, 1),
            ("ab", 2, 2),
            ("ab", 3, 2),
            ("あい", 1, 3),
            ("あい", 2, 6),
            ("éあ", 1, 2),
            ("éあ", 2, 5),
            // A position inside a surrogate pair rounds down to that character's start.
            ("😀😀", 1, 0),
            ("😀😀", 3, 4),
            ("a😀b", 2, 1),
            // End-of-string and oversized positions clamp to the UTF-8 length.
            ("a😀b", 4, 6),
            ("a😀b", 5, 6),
            ("😀😀", 4, 8),
            ("😀😀", usize::MAX, 8),
        ] {
            let byte = utf16_to_utf8_offset(text, units);
            assert_eq!(byte, expected, "{text:?}, UTF-16 offset {units}");
            assert!(text.is_char_boundary(byte));
        }
    }

    #[test]
    fn cursor_offsets_match_decoded_utf16_prefixes() {
        for text in ["", "ab", "あい", "éあ", "😀😀", "a😀b", "👨‍👩‍👧🇯🇵", "e\u{301}", "مرحبا"]
        {
            let encoded: Vec<u16> = text.encode_utf16().collect();
            let mut previous = 0;
            for units in 0..=encoded.len() + 3 {
                // An independent oracle: truncate the UTF-16 representation, drop any
                // trailing high surrogate, then decode the complete prefix back to UTF-8.
                let mut prefix = encoded[..units.min(encoded.len())].to_vec();
                if matches!(prefix.last(), Some(0xd800..=0xdbff)) {
                    prefix.pop();
                }
                let expected = String::from_utf16(&prefix).unwrap().len();
                let byte = utf16_to_utf8_offset(text, units);
                assert_eq!(byte, expected, "{text:?}, UTF-16 offset {units}");
                assert!(text.is_char_boundary(byte));
                assert!(byte >= previous);
                previous = byte;
            }
        }
    }
}
