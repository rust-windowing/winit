#![cfg(target_os = "redox")]

use std::sync::{Arc, Mutex};

use Window;

pub trait WindowExt {
    fn get_orbclient_window(&self) -> Arc<Mutex<orbclient::Window>>;
}

impl WindowExt for Window {
    fn get_orbclient_window(&self) -> Arc<Mutex<orbclient::Window>> {
        self.window.get_orbclient_window()
    }
}
