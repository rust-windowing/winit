//! Write events to an output stream.
//!
//! The format is as follows:
//! - First 8 bytes: big-endian length of payload.
//! - Next {len} bytes: JSON payload to deserialize from.

use crate::{TestEvent, TestHandler};
use std::io::Write;

/// A wrapper around a writer that sends data down a stream.
#[derive(Debug)]
pub struct WriteHandler<W: Write> {
    /// The inner writer.
    writer: W,
}

impl<W: Write> WriteHandler<W> {
    /// Create a new write handler.
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

impl<W: Write> TestHandler for WriteHandler<W> {
    fn handle_test(&mut self, event: TestEvent) {
        let payload = serde_json::to_vec(&event).unwrap();
        let length = u64::to_be_bytes(payload.len() as u64);

        // Write the payload to the stream.
        self.writer.write_all(&length).unwrap();
        self.writer.write_all(&payload).unwrap();
    }
}

impl<W: Write> Drop for WriteHandler<W> {
    fn drop(&mut self) {
        self.writer.flush().ok();
    }
}
