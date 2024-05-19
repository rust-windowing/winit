use std::error::Error;
use std::fs::File;

use winit::application::ApplicationHandler;
use winit::event_loop::{ActiveEventLoop, EventLoop};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
enum UserEvent {
    WakeUp,
    Counter(u64),
}

struct Application {
    file: std::fs::File,
}

impl Application {
    fn new<T>(_event_loop: &EventLoop<T>) -> Self {
        Self {
            file: File::options()
                .write(true)
                .truncate(true)
                .append(false)
                .create(true)
                .open("log.txt")
                .unwrap(),
        }
    }
}

impl ApplicationHandler<UserEvent> for Application {
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        // write events to file, leave stdout for other info
        use std::io::Write;
        writeln!(&mut self.file, "User event: {event:?}").unwrap();

        if let UserEvent::Counter(c) = event {
            if c == 15000 {
                std::process::exit(0);
            }
        }
    }

    fn resumed(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {}

    fn window_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        _event: winit::event::WindowEvent,
    ) {
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();

    std::thread::spawn(move || {
        let mut counter = 0;
        loop {
            if proxy.send_event(UserEvent::Counter(counter)).is_err() {
                println!("Failed: {}", counter);
            }

            counter += 1;

            if counter > 15000 {
                let mut wakeup_counter = 1;
                loop {
                    let _ = proxy.send_event(UserEvent::WakeUp);
                    println!("Sent {wakeup_counter} WakeUp events");
                    wakeup_counter += 1;
                }
            }
        }
    });

    let mut state = Application::new(&event_loop);

    event_loop.run_app(&mut state).map_err(Into::into)
}
