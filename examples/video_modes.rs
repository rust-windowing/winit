use winit::event_loop::EventLoop;

fn main() {
    simple_logger::init().unwrap();
    let event_loop = EventLoop::new();
    let monitor = event_loop.primary_monitor();

    println!("Listing available video modes:");

    for mode in monitor.video_modes() {
        println!("{}", mode);
    }
}
