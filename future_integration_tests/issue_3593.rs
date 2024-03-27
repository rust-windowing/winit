use winit::event_loop::EventLoop;
use winit::window::Window;
use rwh_06::HasWindowHandle;

fn issue_3593() {
    // We should not be able to get a window handle off of the main thread.
    let event_loop = EventLoop::new().unwrap();
    event_loop.run(|_, elwt| {
        // Should work on our thread.
        let window = Window::new(&elwt).unwrap();
        assert!(window.window_handle().is_ok());

        // Should't work on another thread.
        std::thread::spawn(move || {
            if cfg!(windows) {
                assert!(window.window_handle().is_err());
            } else {
                assert!(window.window_handle().is_ok());
            }
        }).join().unwrap();

        elwt.exit();
    });
}
