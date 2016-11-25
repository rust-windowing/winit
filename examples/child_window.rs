extern crate winit;
use std::thread;
use winit::os::windows::WindowBuilderExt;

fn resize_callback(width: u32, height: u32) {
    println!("Window resized to {}x{}", width, height);
}

fn main() {
    let window = winit::WindowBuilder::new()
        .with_title("A fantastic window!")
        .with_window_resize_callback(resize_callback)
        .build()
        .unwrap();

    let proxy = window.create_window_proxy();
    thread::spawn(move || {
        let child = winit::WindowBuilder::new()
            .with_title("child window!")
            .with_window_resize_callback(resize_callback)
            .with_decorations(false)
            .with_parent_window(proxy)
            .build()
            .unwrap();

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
}
