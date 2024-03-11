// Copyright 2024 The Winit Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Read test events from a stream.

use gui_test::{TestEvent, TestEventType};
use std::io::{self, Read};

/// Read events from a stream.
pub(super) struct StreamReader<R> {
    /// The inner reader.
    reader: Option<R>,

    /// Reused buffer.
    buffer: Vec<u8>,
}

impl<R: Read> StreamReader<R> {
    /// Create a new stream reader.
    pub(super) fn new(reader: R) -> Self {
        Self {
            reader: Some(reader),
            buffer: vec![0u8; 1024],
        }
    }
}

macro_rules! leap {
    ($self:expr, $e:expr) => {{
        match ($e) {
            Ok(x) => x,
            Err(err) => {
                ($self).reader = None;
                return Some(Err(err));
            }
        }
    }};
}

impl<R: Read> Iterator for StreamReader<R> {
    type Item = io::Result<TestEvent>;

    fn next(&mut self) -> Option<Self::Item> {
        let reader = self.reader.as_mut()?;

        // Read eight bytes from the reader to get payload length.
        let mut len_buffer = [0u8; 8];
        leap!(self, reader.read_exact(&mut len_buffer));

        // Parse that, then read the length's worth of bytes.
        let length = u64::from_be_bytes(len_buffer);
        self.buffer.resize(length as usize, 0);
        leap!(self, reader.read_exact(&mut self.buffer));

        // Parse as a test event.
        let event: TestEvent = leap!(
            self,
            serde_json::from_slice(&self.buffer)
                .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
        );

        // If this is complete, stop running.
        if matches!(event.ty, TestEventType::Complete { .. }) {
            self.reader = None;
        }

        // We are okay.
        Some(Ok(event))
    }
}
