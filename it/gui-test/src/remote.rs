//! Create a test handler that can be run remotely.

use crate::TestHandler;
use crate::stream::WriteHandler;
use crate::user::UserHandler;

use std::env;
use std::net::TcpStream;

/// Create a test handler adjusted for the current environment.
pub fn handler() -> Box<dyn TestHandler + Send + 'static> {
    // If GUI_TEST_UNIX_STREAM is enabled, use that as a Unix stream.
    #[cfg(unix)]
    if let Some(stream_path) = env::var_os("GUI_TEST_UNIX_STREAM")
        .filter(|s| !s.is_empty()) {
        let stream = std::os::unix::net::UnixStream::connect(stream_path).expect("unable to connect to gui-test handler");
        return Box::new(WriteHandler::new(stream));
    }

    // If GUI_TEST_TCP_STREAM is enabled, use that as a TCP stream.
    if let Some(tcp_ip) = env::var("GUI_TEST_TCP_STREAM")
        .ok()
        .filter(|s| !s.is_empty()) {
        let stream = TcpStream::connect(tcp_ip).unwrap();
        return Box::new(WriteHandler::new(stream));
    }

    // By default, use the user handler.
    Box::new(UserHandler::new())
}
