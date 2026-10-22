//! Local file names from RFC 2483 URI lists and RFC 8089 file URIs.

use std::io;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

pub(super) const MAX_BYTES: usize = 1024 * 1024;
const MAX_FILES: usize = 256;

/// Decode only local file URIs. A rejected offer is never interpreted as a network location,
/// shell command or a relative path in the receiving process's working directory.
pub(super) fn file_paths(bytes: &[u8]) -> io::Result<Vec<PathBuf>> {
    let bad = || io::Error::new(io::ErrorKind::InvalidData, "invalid local file drop");
    if bytes.len() > MAX_BYTES {
        return Err(bad());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| bad())?;
    let mut paths = Vec::new();
    for line in text.lines().map(str::trim).filter(|s| !s.is_empty() && !s.starts_with('#')) {
        let (scheme, rest) = line.split_once(':').ok_or_else(bad)?;
        if !scheme.eq_ignore_ascii_case("file") {
            return Err(bad());
        }
        let path = if let Some(authority) = rest.strip_prefix("//") {
            let (host, path) = authority.split_once('/').ok_or_else(bad)?;
            if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
                return Err(bad());
            }
            format!("/{path}")
        } else if rest.starts_with('/') {
            rest.to_owned()
        } else {
            return Err(bad());
        };
        // Fragments/queries must be percent-encoded when they are part of the file name.
        if path.contains(['#', '?']) || paths.len() == MAX_FILES {
            return Err(bad());
        }
        let mut remaining = path.as_bytes();
        while let Some(at) = remaining.iter().position(|b| *b == b'%') {
            let tail = remaining.get(at + 1..).ok_or_else(bad)?;
            if !tail.get(..2).is_some_and(|pair| pair.iter().all(u8::is_ascii_hexdigit)) {
                return Err(bad());
            }
            remaining = tail.get(2..).ok_or_else(bad)?;
        }
        let decoded: Vec<u8> = percent_encoding::percent_decode_str(&path).collect();
        if decoded.contains(&0) {
            return Err(bad());
        }
        paths.push(PathBuf::from(std::ffi::OsString::from_vec(decoded)));
    }
    if paths.is_empty() {
        return Err(bad());
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStrExt;

    #[test]
    fn local_names_keep_spaces_unicode_punctuation_and_unix_bytes() {
        let paths = file_paths(b"# selection\r\nfile:///tmp/a%20b.png\r\nFILE://LOCALHOST/tmp/%E2%9C%93%23%3F.png\nfile:/tmp/%FF.jpg\n").unwrap();
        assert_eq!(paths[0], PathBuf::from("/tmp/a b.png"));
        assert_eq!(paths[1], PathBuf::from("/tmp/\u{2713}#?.png"));
        assert_eq!(paths[2].as_os_str().as_bytes(), b"/tmp/\xff.jpg");
    }

    #[test]
    fn unsafe_or_ambiguous_locations_are_not_file_drops() {
        for input in [
            "",
            "# comment",
            "https://example.org/a.png",
            "file://remote/a.png",
            "file://user@localhost/a.png",
            "file:relative.png",
            "file:///tmp/%00",
            "file:///tmp/%",
            "file:///tmp/%GG",
            "file:///tmp/a#fragment",
            "file:///tmp/a?query",
        ] {
            assert!(file_paths(input.as_bytes()).is_err(), "{input}");
        }
    }

    #[test]
    fn lists_are_bounded_before_decoding_or_allocating_paths() {
        assert!(file_paths(&vec![b'x'; MAX_BYTES + 1]).is_err());
        let one = "file:///tmp/a.png\n";
        assert_eq!(file_paths(one.repeat(MAX_FILES).as_bytes()).unwrap().len(), MAX_FILES);
        assert!(file_paths(one.repeat(MAX_FILES + 1).as_bytes()).is_err());
    }
}
