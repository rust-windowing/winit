extern crate winit;

fn main() {
    let events_loop = winit::EventsLoop::new();

    let _window = winit::WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&events_loop)
        .unwrap();

    events_loop.run_forever(|event| {
        println!("{:?}", event);

        match event {
            winit::Event::WindowEvent { event: winit::WindowEvent::Closed, .. } => events_loop.interrupt(),
            _ => ()
        }
    });
}
