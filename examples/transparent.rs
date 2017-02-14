extern crate winit;

fn main() {
    let events_loop = winit::EventsLoop::new();

    let window = winit::WindowBuilder::new()
        .with_decorations(false)
        .with_transparency(true)
        .build(&events_loop)
        .unwrap();

    window.set_title("A fantastic window!");

    events_loop.run_forever(|event| {
        println!("{:?}", event);

        match event {
            winit::Event::WindowEvent { event: winit::WindowEvent::Closed, .. } => {
                events_loop.interrupt()
            }
            _ => (),
        }
    });
}
