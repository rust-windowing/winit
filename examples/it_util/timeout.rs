// Spawns a timeout thread that will close the event loop after one second.

#[cfg(not(wasm_platform))]
mod util {
    use std::env;
    use std::thread;
    use winit::event_loop::EventLoop;

    pub(super) fn start_timeout_thread<T: Send + 'static>(event_loop: &EventLoop<T>, msg_to_send: T) {
        // If the WINIT_EXAMPLE_TIMEOUT environment variable is set, get the number of seconds
        // to wait before closing the window.
        let secs = match env::var("WINIT_EXAMPLE_TIMEOUT")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
        {
            Some(secs) => secs,
            None => return,
        };

        // Spawn a thread that will close the window after `secs` seconds.
        thread::Builder::new()
            .name("winit example timeout".to_string())
            .spawn({
                let proxy = event_loop.create_proxy();
                move || {
                    thread::sleep(std::time::Duration::from_secs(secs));
                    println!("Closing window due to timeout");
                    proxy.send_event(msg_to_send).unwrap_or_else(|_| panic!("Failed to send event to event loop"));
                }
            })
            .expect("failed to spawn timeout thread");
    }
}

#[cfg(wasm_platform)]
mod util {
    use winit::event_loop::EventLoop;

    pub(super) fn start_timeout_thread<T: Send + 'static>(_event_loop: &EventLoop<T>, _msg_to_send: T) {
        // Not supported on web.
    }
}
