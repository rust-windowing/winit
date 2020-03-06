use winit::event_loop::EventLoop;

fn main() {
    simple_logger::init().unwrap();
    let event_loop = EventLoop::new();

    println!("Listing available video modes:");

    for monitor in event_loop.available_monitors() {
        println!("{:?}", monitor);
        for mode in monitor.video_modes() {
            println!("{}", mode);
        }
    }
}
