use std::ffi::OsString;
use std::io::{BufRead, Cursor, Read, Write};
use std::mem;

#[derive(Default, Debug)]
pub struct UriListEncoder {
    uris: <Vec<OsString> as IntoIterator>::IntoIter,
    // Weird system with two fields since otherwise we get lifetime errors.
    uri_reader: Cursor<Vec<u8>>,
    newline_reader: Cursor<&'static [u8]>,
}

fn percent_encode_into(out: &mut Vec<u8>, value: &[u8]) {
    /// Characters that are invalid in a URI
    const URI_RESERVED: &percent_encoding::AsciiSet = &percent_encoding::CONTROLS
        .add(b' ')
        .add(b'!')
        .add(b'"')
        .add(b'$')
        .add(b'%')
        .add(b'\'')
        .add(b'(')
        .add(b')')
        .add(b'*')
        .add(b'+')
        .add(b',')
        .add(b';')
        .add(b'<')
        .add(b'>')
        .add(b'@')
        .add(b'[')
        .add(b'\\')
        .add(b']')
        .add(b'^')
        .add(b'`')
        .add(b'{')
        .add(b'|')
        .add(b'}')
        .add(b'~');

    for slice in percent_encoding::percent_encode(value, URI_RESERVED) {
        out.extend_from_slice(slice.as_bytes());
    }
}

impl From<Vec<OsString>> for UriListEncoder {
    fn from(value: Vec<OsString>) -> Self {
        let mut uris = value.into_iter();

        let Some(first_uri) = uris.next() else {
            return Default::default();
        };

        let first_uri_bytes = first_uri.as_encoded_bytes();

        let mut bytes = Vec::with_capacity(first_uri_bytes.len());

        percent_encode_into(&mut bytes, first_uri_bytes);

        Self { uris, uri_reader: Cursor::new(bytes), newline_reader: Cursor::new(b"\r\n") }
    }
}

impl Read for UriListEncoder {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut writer = Cursor::new(buf);

        let mut total = 0;

        loop {
            let buf = self.fill_buf()?;
            let written_amount = writer.write(buf)?;

            if written_amount == 0 {
                break Ok(total);
            }

            self.consume(written_amount);

            total += written_amount;
        }
    }
}

impl BufRead for UriListEncoder {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        if let Some(uri_buf) = Some(self.uri_reader.fill_buf()?).filter(|buf| !buf.is_empty()) {
            return Ok(uri_buf);
        }

        self.newline_reader.fill_buf()
    }

    fn consume(&mut self, mut amount: usize) {
        let uri_total_len = self.uri_reader.get_ref().len();
        let uri_remaining = uri_total_len - self.uri_reader.position() as usize;
        let nl_total_len = self.newline_reader.get_ref().len();
        let nl_remaining = nl_total_len - self.newline_reader.position() as usize;

        self.uri_reader.consume(amount.min(uri_remaining));
        amount = amount.saturating_sub(uri_remaining);

        self.newline_reader.consume(amount.min(nl_remaining));
        amount = amount.saturating_sub(nl_remaining);

        if amount == 0 {
            return;
        }

        let Some(next_uri) = self.uris.next() else {
            return;
        };

        let mut bytes = mem::take(self.uri_reader.get_mut());

        bytes.clear();

        percent_encode_into(&mut bytes, next_uri.as_encoded_bytes());

        self.uri_reader = Cursor::new(bytes);
        self.newline_reader.set_position(0);

        self.consume(amount);
    }
}

#[derive(Debug)]
pub enum SendDataEncoder {
    Uris(UriListEncoder),
    Bytes(Cursor<Vec<u8>>),
}

impl Read for SendDataEncoder {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            SendDataEncoder::Uris(inner) => inner.read(buf),
            SendDataEncoder::Bytes(inner) => inner.read(buf),
        }
    }
}

impl BufRead for SendDataEncoder {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        match self {
            SendDataEncoder::Uris(inner) => inner.fill_buf(),
            SendDataEncoder::Bytes(inner) => inner.fill_buf(),
        }
    }

    fn consume(&mut self, amount: usize) {
        match self {
            SendDataEncoder::Uris(inner) => inner.consume(amount),
            SendDataEncoder::Bytes(inner) => inner.consume(amount),
        }
    }
}
