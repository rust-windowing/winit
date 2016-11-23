extern crate winit;

use std::thread;

fn main() {
    let window1 = winit::WindowBuilder::new().build().unwrap();
    let window2 = winit::WindowBuilder::new().build().unwrap();
    let window3 = winit::WindowBuilder::new().build().unwrap();

    let t1 = thread::spawn(move || {
        run(window1);
    });

    let t2 = thread::spawn(move || {
        run(window2);
    });

    let t3 = thread::spawn(move || {
        run(window3);
    });

    let _ = t1.join();
    let _ = t2.join();
    let _ = t3.join();
}

fn run(window: winit::Window) {
    for event in window.wait_events() {
        match event {
            winit::Event::Closed => break,
            _ => ()
        }
    }
}
