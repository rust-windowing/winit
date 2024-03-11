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

//! Run the actual Docker command.

use crate::command::Command;

use camino::Utf8Path;

use std::ffi::OsStr;
use std::io;
use std::process::Child;

/// The Docker command line.
pub(super) struct DockerRun {
    command: Command,
}

impl DockerRun {
    /// Start the command.
    pub(super) fn new() -> Self {
        let mut command = Command::new("docker");
        command.arg("run");

        Self { command }
    }

    /// Run with an environment variable.
    pub(super) fn env(&mut self, name: impl AsRef<str>, value: impl AsRef<str>) -> &mut Self {
        let env_arg = format!("{}={}", name.as_ref(), value.as_ref());
        self.command.args(["--env", &env_arg]);
        self
    }

    /// Run with a simple `init` process.
    pub(super) fn init(&mut self) -> &mut Self {
        self.command.arg("--init");
        self
    }

    /// Set the working directory.
    pub(super) fn workdir(&mut self, dir: impl AsRef<OsStr>) -> &mut Self {
        self.command.arg("--workdir");
        self.command.arg(dir);
        self
    }

    /// Remove the container once it is complete.
    pub(super) fn rm(&mut self) -> &mut Self {
        self.command.arg("--rm");
        self
    }

    /// Pass a volume into the container.
    pub(super) fn volume(
        &mut self,
        host: impl AsRef<Utf8Path>,
        container: impl AsRef<Utf8Path>,
    ) -> &mut Self {
        let list = format!("{}:{}", host.as_ref(), container.as_ref());
        self.command.args(["--volume", &list]);
        self
    }

    /// Run the container with a command.
    pub(super) fn run_with_command<T: AsRef<OsStr>>(
        &mut self,
        container_name: impl AsRef<str>,
        container_version: impl AsRef<str>,
        command: impl IntoIterator<Item = T>,
    ) -> io::Result<Child> {
        self.command.arg(format!(
            "{}:{}",
            container_name.as_ref(),
            container_version.as_ref()
        ));
        self.command.args(command);
        self.command.spawn()
    }
}
