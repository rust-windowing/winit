fn main() {
    child_window_exemple::child_window();
}

#[cfg(windows)]
mod child_window_exemple {

    extern crate winit;
    extern crate winapi;
    use std::thread;
    use self::winit::os::windows::{WindowBuilderExt, WindowExt};

    fn resize_callback(width: u32, height: u32) {
        println!("Window resized to {}x{}", width, height);
    }
    /**
    * Creates a main window and a child within it and handle their events separetely.
    * Currently windows only
    */
    pub fn child_window() {
        let window = winit::WindowBuilder::new()
            .with_title("A fantastic window!")
            .with_window_resize_callback(resize_callback)
            .build()
            .unwrap();

        let parent = window.get_hwnd() as winapi::HWND;
        let child = winit::WindowBuilder::new()
            .with_title("child window!")
            .with_window_resize_callback(resize_callback)
            .with_decorations(false)
            .with_dimensions(100, 100)
            .with_parent_window(parent)
            .build()
            .unwrap();

        let child_thread = thread::spawn(move || {
            for event in child.wait_events() {
                println!("child {:?}", event);

                match event {
                    winit::Event::Closed => break,
                    _ => (),
                }
            }
        });

        for event in window.wait_events() {
            println!("parent {:?}", event);

            match event {
                winit::Event::Closed => break,
                _ => (),
            }
        }

        child_thread.join().unwrap();
    }
}

#[cfg(not(windows))]
mod child_window_exemple {
    pub fn child_window() {}
}
