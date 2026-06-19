use std::error::Error;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;

use dpi::PhysicalPosition;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, RgbImage};
use softbuffer::{Context, Surface};
use tracing::{error, info, warn};
use winit::application::ApplicationHandler;
use winit::data_transfer::{DataTransferId, DataTransferSendBuilder, TypeHint};
use winit::event::{ButtonSource, MouseButton, WindowEvent};
use winit::event_loop::{
    ActiveEventLoop, AsyncRequestSerial, DndAction, DragIcon, EventLoop, OwnedDisplayHandle,
};
use winit::icon::{Icon, RgbaIcon};
use winit::window::{Window, WindowAttributes, WindowId};

#[path = "util/fill.rs"]
mod fill;
#[path = "util/tracing.rs"]
mod tracing;

fn main() -> Result<(), Box<dyn Error>> {
    tracing::init();

    let event_loop = EventLoop::new()?;

    let app = Application::new();
    Ok(event_loop.run_app(app)?)
}

/// Application state and event handling.
#[derive(Debug)]
struct Application {
    surface: Option<Surface<OwnedDisplayHandle, Box<dyn Window>>>,
    last_dnd_fetch: Option<AsyncRequestSerial>,
    last_drag_start: Option<DataTransferId>,
    drag_icon: (Icon, PhysicalPosition<i32>),
    drag_image_data: Arc<RgbImage>,
}

const DRAG_IMAGE: &[u8] = include_bytes!("data/icon.png");

impl Application {
    fn new() -> Self {
        let drag_icon = load_icon(DRAG_IMAGE);
        let drag_image_data = Arc::new(image::load_from_memory(DRAG_IMAGE).unwrap().into_rgb8());
        Self {
            surface: None,
            last_dnd_fetch: None,
            last_drag_start: None,
            drag_icon,
            drag_image_data,
        }
    }
}

fn load_icon(bytes: &[u8]) -> (Icon, PhysicalPosition<i32>) {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::load_from_memory(bytes).unwrap().into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    (
        RgbaIcon::new(icon_rgba, icon_width, icon_height).expect("Failed to open icon").into(),
        PhysicalPosition { x: -(icon_width as i32) / 2, y: -(icon_height as i32) / 2 },
    )
}

impl ApplicationHandler for Application {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        let window_attributes =
            WindowAttributes::default().with_title("Drag and drop files, text or HTML onto me!");

        let window = event_loop.create_window(window_attributes).unwrap();
        let context = Context::new(event_loop.owned_display_handle()).unwrap();
        let surface = Surface::new(&context, window).unwrap();
        self.surface = Some(surface);
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::PointerButton { button: ButtonSource::Mouse(button), state, .. }
                if button == MouseButton::Left && state.is_pressed() =>
            {
                let (icon, offset) = self.drag_icon.clone();

                // In a real application, you probably wouldn't advertise so many types.
                // Depending on platform and destination application, different options may be
                // chosen.
                let result = event_loop.start_drag(
                    window_id,
                    DataTransferSendBuilder::new(self.drag_image_data.clone())
                        .with_type(TypeHint::UriList, |_, _| {
                            let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                            let root = manifest_dir.parent().unwrap();
                            let this_file = root.join(file!());
                            let icon_file = this_file.parent().unwrap().join("data/icon.png");
                            let icon_file = icon_file.display();
                            Some(vec![OsString::from(format!("file://{icon_file}"))])
                        })
                        .with_type(TypeHint::Plaintext, |_, _| Some("Winit example".to_string()))
                        .with_type(TypeHint::Html, |_, _| {
                            Some("<span><strong>Winit</strong> example</span>".to_string())
                        })
                        // You can advertise a `TypeHint` that can match many types, and switch
                        // inside the callback. For example, this will match any image type.
                        // This may be desirable on some platforms which restrict the set of
                        // image types that can be sent.
                        .with_type(TypeHint::Image { extension_hint: None }, |image, ty| {
                            let hint = ty.hint()?;
                            match hint {
                                TypeHint::Image { extension_hint } => {
                                    let image = DynamicImage::from((**image).clone());
                                    let (w, h) = image.dimensions();
                                    let image = image
                                        .resize(w * 8, h * 8, FilterType::Gaussian)
                                        .into_rgb8();
                                    let ext = extension_hint.unwrap_or("png");
                                    info!("Destination requested image as {ext}, converting...");
                                    let format = image::ImageFormat::from_extension(ext)?;
                                    let mut out_buf = Vec::new();
                                    let mut out_writer = std::io::Cursor::new(&mut out_buf);

                                    image.write_to(&mut out_writer, format).ok()?;

                                    Some(out_buf)
                                },
                                _ => None,
                            }
                        })
                        .build(),
                    &[DndAction::Move, DndAction::Copy],
                    Some(DragIcon { icon, offset }),
                );

                self.last_drag_start = result.ok();
            },
            WindowEvent::DragLeft { .. } => {
                info!("{event:?}");
                self.last_dnd_fetch = None;
            },
            WindowEvent::DataTransferReceived { ref value, serial, .. } => {
                assert_eq!(self.last_dnd_fetch, Some(serial));

                match value.type_().hint() {
                    Some(TypeHint::Plaintext | TypeHint::Html) => {
                        let Ok(text) = value.try_as_string() else {
                            return;
                        };
                        info!("{text:?}");
                    },
                    Some(TypeHint::UriList) => {
                        let Ok(uris) = value.try_as_uris() else {
                            return;
                        };
                        info!("{uris:#?}");
                    },
                    Some(TypeHint::Image { extension_hint: ext }) => {
                        let Ok(bytes) = value.try_as_bytes() else {
                            return;
                        };
                        let format = ext.and_then(image::ImageFormat::from_extension);

                        let reader = std::io::Cursor::new(&bytes[..]);
                        let reader = match format {
                            Some(fmt) => image::ImageReader::with_format(reader, fmt),
                            None => image::ImageReader::new(reader),
                        };

                        match reader.decode() {
                            Ok(image) => {
                                let width = image.width();
                                let height = image.height();
                                info!("Received image ({width}x{height})");
                            },
                            Err(err) => {
                                warn!("Failed to decode image: {err}");
                            },
                        }
                    },
                    _ => {
                        unreachable!("Received a type we didn't ask for!");
                    },
                }
            },
            WindowEvent::DragPosition { .. } => {
                info!("{event:?}");
            },
            WindowEvent::DragDropped { .. } => {
                info!("{event:?}");
            },
            WindowEvent::DragEntered { id, .. } => {
                info!("{event:?}");

                let data_transfer = match event_loop.data_transfer(id) {
                    Ok(dt) => dt,
                    Err(e) => {
                        error!("{e}");
                        return;
                    },
                };

                info!("Types: {:#?}", data_transfer.available_types());

                let readable_image_types = image::ImageFormat::all()
                    .filter(|fmt| fmt.reading_enabled())
                    .filter_map(|fmt| {
                        let ext = fmt.extensions_str().first()?;

                        Some(TypeHint::Image { extension_hint: Some(ext) })
                    });

                let mut valid_types = readable_image_types.chain([
                    TypeHint::Html,
                    TypeHint::UriList,
                    TypeHint::Plaintext,
                ]);

                let valid_type = valid_types.find(|ty| data_transfer.has_type(ty));

                let Some(type_) = valid_type else {
                    event_loop.set_valid_dnd_actions(id, &[]).unwrap();
                    return;
                };

                event_loop.set_valid_dnd_actions(id, &[DndAction::Move, DndAction::Copy]).unwrap();

                self.last_dnd_fetch = event_loop.fetch_data_transfer(id, &type_).ok();
            },
            WindowEvent::OutgoingDragEnded { .. } => {
                info!("{event:?}");
            },
            WindowEvent::RedrawRequested => {
                let surface = self.surface.as_mut().unwrap();
                surface.window().pre_present_notify();
                fill::fill(surface);
            },
            WindowEvent::CloseRequested => {
                event_loop.exit();
            },
            _ => {},
        }
    }
}
