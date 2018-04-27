// Heads up: you need to compile this example with `--features icon_loading`.
// `Icon::from_path` won't be available otherwise, though for your own applications, you could use
// `Icon::from_rgba` if you don't want to depend on the `image` crate.

extern crate winit;
#[cfg(feature = "icon_loading")]
extern crate image;

use winit::Icon;

#[cfg(feature = "icon_loading")]
fn main() {
    // You'll have to choose an icon size at your own discretion. On X11, the desired size varies
    // by WM, and on Windows, you still have to account for screen scaling. Here we use 32px,
    // since it seems to work well enough in most cases. Be careful about going too high, or
    // you'll be bitten by the low-quality downscaling built into the WM.
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/icon.png");
    // While `Icon::from_path` is the most straightforward, you have a few other options. If you
    // want to use the `include_bytes` macro, then pass the result to `Icon::from_bytes`. See the
    // docs for the full list of options (you'll have to generate the docs with the `icon_loading`
    // feature enabled).
    let icon = Icon::from_path(path).expect("Failed to open icon");

    let mut events_loop = winit::EventsLoop::new();

    let window = winit::WindowBuilder::new()
        .with_title("An iconic window!")
        // At present, this only does anything on Windows and X11, so if you want to save load
        // time, you can put icon loading behind a function that returns `None` on other platforms.
        .with_window_icon(Some(icon))
        .build(&events_loop)
        .unwrap();

    events_loop.run_forever(|event| {
        if let winit::Event::WindowEvent { event, .. } = event {
            use winit::WindowEvent::*;
            match event {
                CloseRequested => return winit::ControlFlow::Break,
                DroppedFile(path) => {
                    use image::GenericImage;

                    let icon_image = image::open(path).expect("Failed to open window icon");

                    let (width, height) = icon_image.dimensions();
                    const DESIRED_SIZE: u32 = 32;
                    let (new_width, new_height) = if width == height {
                        (DESIRED_SIZE, DESIRED_SIZE)
                    } else {
                        // Note that this will never divide by zero, due to the previous condition.
                        let aspect_adjustment = DESIRED_SIZE as f64
                            / std::cmp::max(width, height) as f64;
                        (
                            (width as f64 * aspect_adjustment) as u32,
                            (height as f64 * aspect_adjustment) as u32,
                        )
                    };

                    // By scaling the icon ourselves, we get higher-quality filtering and save
                    // some memory.
                    let icon = image::imageops::resize(
                        &icon_image,
                        new_width,
                        new_height,
                        image::FilterType::Lanczos3,
                    );

                    let (offset_x, offset_y) = (
                        (DESIRED_SIZE - new_width) / 2,
                        (DESIRED_SIZE - new_height) / 2,
                    );

                    let mut canvas = image::ImageBuffer::new(DESIRED_SIZE, DESIRED_SIZE);
                    image::imageops::replace(
                        &mut canvas,
                        &icon,
                        offset_x,
                        offset_y,
                    );

                    window.set_window_icon(Some(canvas.into()));
                },
                _ => (),
            }
        }
        winit::ControlFlow::Continue
    });
}

#[cfg(not(feature = "icon_loading"))]
fn main() {
    print!(
r#"This example requires the `icon_loading` feature:
    cargo run --example window_icon --features icon_loading
"#);
}
