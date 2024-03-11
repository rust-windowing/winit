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

//! A wrapper around the `Command` type that dumps the command to stderr.
//!
//! Essentially it's like `set -x` in Bash.

use std::ffi::{OsStr, OsString};
use std::io::{self, prelude::*};
use std::process::Child;

/// Simple `Command` wrapper.
pub(super) struct Command {
    /// Actual inner command.
    inner: std::process::Command,

    /// Command to run.
    text: Vec<OsString>,
}

impl Command {
    /// Create a new `Command`.
    pub(super) fn new(cmd: impl AsRef<OsStr>) -> Self {
        let cmd = cmd.as_ref();
        Self {
            inner: std::process::Command::new(cmd),
            text: vec![cmd.to_os_string()],
        }
    }

    /// Add an argument to the `Command`.
    pub(super) fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        let arg = arg.as_ref();
        self.inner.arg(arg);
        self.text.push(arg.to_os_string());
        self
    }

    /// Add multiple arguments to the `Command`.
    pub(super) fn args<T: AsRef<OsStr>>(&mut self, args: impl IntoIterator<Item = T>) -> &mut Self {
        for arg in args {
            let arg = arg.as_ref();
            self.inner.arg(arg);
            self.text.push(arg.to_os_string());
        }

        self
    }

    /// Spawn the process.
    pub(super) fn spawn(&mut self) -> io::Result<Child> {
        dump_text(&self.text);
        self.inner.spawn()
    }
}

/// Dump `OsString` list to stderr.
fn dump_text(text: &[OsString]) {
    let mut cerr = io::stderr().lock();
    write!(&mut cerr, "+").unwrap();

    for arg in text {
        match arg.to_str() {
            Some(arg) => write!(&mut cerr, " {}", arg).unwrap(),
            None => write!(&mut cerr, " {:?}", arg).unwrap(),
        }
    }

    writeln!(&mut cerr).unwrap();
}
