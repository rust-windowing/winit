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

//! Run tests inside of a Linux docker container.

use super::command::DockerRun;
use crate::stream::StreamReader;

use gui_test::remote::handler;
use gui_test::TestHandler;

use std::io;
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::thread;

const UBUNTU_DOCKERFILE: &str = "ghcr.io/rust-windowing/testubuntu";
const LATEST: &str = "latest";

/// Run the provided test in a Linux docker container.
pub(crate) fn linux_test(test_name: &str) -> io::Result<()> {
    // Create a Unix socket to listen for events on.
    let unix_path = format!("/tmp/gui_test_{}.sock", fastrand::u16(..));
    let listener = UnixListener::bind(&unix_path)?;

    // Spawn the Docker container.
    let mut container = {
        let mut docker = DockerRun::new();

        // Usual options.
        docker.rm().init();

        // Pass through the socket as a volume.
        docker.volume(&unix_path, &unix_path);

        // Pass through the winit directory.
        let winit_directory = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .find_map(|path| {
                let cargo_toml = path.join("Cargo.toml");
                let contents = std::fs::read(cargo_toml).ok()?;

                if std::str::from_utf8(&contents)
                    .ok()?
                    .contains("name = \"winit\"")
                {
                    Some(path)
                } else {
                    None
                }
            })
            .unwrap();
        docker.volume(
            camino::Utf8Path::from_path(winit_directory).unwrap(),
            "/app/winit/",
        );

        // Set the working dir to this directory.
        docker.workdir("/app/winit/");

        // Set GUI_TEST_UNIX_STREAM to the socket.
        docker.env("GUI_TEST_UNIX_STREAM", &unix_path);

        // Set CARGO_TARGET_DIR to a random other directory.
        docker.env("CARGO_TARGET_DIR", "/tmp/");

        // The command to run the test.
        let command = ["xvfb-run", "cargo", "run", "-p", test_name];

        // Spawn the test container.
        docker.run_with_command(UBUNTU_DOCKERFILE, LATEST, command)?
    };

    // Run the console listener in another thread.
    let handle = thread::spawn(move || {
        // Attach to the listener.
        let (event_reader, _) = listener.accept().unwrap();

        // Read events and output them as we get them.
        let input = StreamReader::new(event_reader);
        let mut output = handler();

        for event in input {
            let event = event?;
            output.handle_test(event);
        }

        io::Result::Ok(())
    });

    // Wait for the container to finish.
    if !container.wait()?.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "docker exited with a failure exit code",
        ));
    }

    // Stop the thread.
    handle.join().unwrap().unwrap();

    Ok(())
}
