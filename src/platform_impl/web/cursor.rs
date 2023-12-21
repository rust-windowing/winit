use std::{
    cell::{Cell, RefCell},
    ops::Deref,
    rc::{Rc, Weak},
};

use crate::cursor::{BadImage, CursorImage};
use cursor_icon::CursorIcon;
use wasm_bindgen::{closure::Closure, JsCast};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Blob, Document, HtmlCanvasElement, ImageBitmap, ImageBitmapOptions,
    ImageBitmapRenderingContext, ImageData, PremultiplyAlpha, Url, Window,
};

use super::backend::Style;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebCustomCursor {
    Image(CursorImage),
    Url {
        url: String,
        hotspot_x: u16,
        hotspot_y: u16,
    },
}

impl WebCustomCursor {
    pub fn from_rgba(
        rgba: Vec<u8>,
        width: u16,
        height: u16,
        hotspot_x: u16,
        hotspot_y: u16,
    ) -> Result<Self, BadImage> {
        Ok(Self::Image(CursorImage::from_rgba(
            rgba, width, height, hotspot_x, hotspot_y,
        )?))
    }

    pub(super) fn build(
        &self,
        window: &Window,
        document: &Document,
        style: &Style,
        previous: SelectedCursor,
        cursor_visible: Rc<Cell<bool>>,
    ) -> SelectedCursor {
        let previous = previous.into();

        match self {
            WebCustomCursor::Image(image) => SelectedCursor::Image(CursorImageState::from_image(
                window,
                document.clone(),
                style.clone(),
                image,
                previous,
                cursor_visible,
            )),
            WebCustomCursor::Url {
                url,
                hotspot_x,
                hotspot_y,
            } => {
                let value = previous.style_with_url(url, *hotspot_x, *hotspot_y);

                if cursor_visible.get() {
                    style.set("cursor", &value);
                }

                SelectedCursor::Url {
                    style: value,
                    previous,
                    url: url.clone(),
                    hotspot_x: *hotspot_x,
                    hotspot_y: *hotspot_y,
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum SelectedCursor {
    Named(CursorIcon),
    Url {
        style: String,
        previous: Previous,
        url: String,
        hotspot_x: u16,
        hotspot_y: u16,
    },
    Image(Rc<RefCell<Option<CursorImageState>>>),
}

impl Default for SelectedCursor {
    fn default() -> Self {
        Self::Named(Default::default())
    }
}

impl SelectedCursor {
    pub fn set_style(&self, style: &Style) {
        let value = match self {
            SelectedCursor::Named(icon) => icon.name(),
            SelectedCursor::Url { style, .. } => style,
            SelectedCursor::Image(image) => {
                let image = image.borrow();
                let value = match image.deref().as_ref().unwrap() {
                    CursorImageState::Loading { previous, .. } => previous.style(),
                    CursorImageState::Failed(previous) => previous.style(),
                    CursorImageState::Ready { style, .. } => style,
                };
                return style.set("cursor", value);
            }
        };

        style.set("cursor", value);
    }
}

#[derive(Debug)]
pub enum Previous {
    Named(CursorIcon),
    Url {
        style: String,
        url: String,
        hotspot_x: u16,
        hotspot_y: u16,
    },
    Image {
        style: String,
        image: WebCursorImage,
    },
}

impl Previous {
    fn style(&self) -> &str {
        match self {
            Previous::Named(icon) => icon.name(),
            Previous::Url { style: url, .. } => url,
            Previous::Image { style, .. } => style,
        }
    }

    fn style_with_url(&self, new_url: &str, new_hotspot_x: u16, new_hotspot_y: u16) -> String {
        match self {
            Previous::Named(icon) => format!("url({new_url}) {new_hotspot_x} {new_hotspot_y}, {}", icon.name()),
            Previous::Url {
                url,
                hotspot_x,
                hotspot_y,
                ..
            }
            | Previous::Image {
                image:
                    WebCursorImage {
                        data_url: url,
                        hotspot_x,
                        hotspot_y,
                        ..
                    },
                ..
            } => format!(
                "url({new_url}) {new_hotspot_x} {new_hotspot_y}, url({url}) {hotspot_x} {hotspot_y}, auto",
            ),
        }
    }
}

impl From<SelectedCursor> for Previous {
    fn from(value: SelectedCursor) -> Self {
        match value {
            SelectedCursor::Named(icon) => Self::Named(icon),
            SelectedCursor::Url {
                style,
                url,
                hotspot_x,
                hotspot_y,
                ..
            } => Self::Url {
                style,
                url,
                hotspot_x,
                hotspot_y,
            },
            SelectedCursor::Image(image) => {
                match Rc::try_unwrap(image).unwrap().into_inner().unwrap() {
                    CursorImageState::Loading { previous, .. } => previous,
                    CursorImageState::Failed(previous) => previous,
                    CursorImageState::Ready {
                        style,
                        image: current,
                        ..
                    } => Self::Image {
                        style,
                        image: current,
                    },
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum CursorImageState {
    Loading {
        style: Style,
        cursor_visible: Rc<Cell<bool>>,
        previous: Previous,
        hotspot_x: u16,
        hotspot_y: u16,
    },
    Failed(Previous),
    Ready {
        style: String,
        image: WebCursorImage,
        previous: Previous,
    },
}

impl CursorImageState {
    fn from_image(
        window: &Window,
        document: Document,
        style: Style,
        image: &CursorImage,
        previous: Previous,
        cursor_visible: Rc<Cell<bool>>,
    ) -> Rc<RefCell<Option<Self>>> {
        // Can't create array directly when backed by SharedArrayBuffer.
        // Adapted from https://github.com/rust-windowing/softbuffer/blob/ab7688e2ed2e2eca51b3c4e1863a5bd7fe85800e/src/web.rs#L196-L223
        #[cfg(target_feature = "atomics")]
        let image_data = {
            use js_sys::{Uint8Array, Uint8ClampedArray};
            use wasm_bindgen::prelude::wasm_bindgen;
            use wasm_bindgen::JsValue;

            #[wasm_bindgen]
            extern "C" {
                #[wasm_bindgen(js_namespace = ImageData)]
                type ImageDataExt;
                #[wasm_bindgen(catch, constructor, js_class = ImageData)]
                fn new(array: Uint8ClampedArray, sw: u32) -> Result<ImageDataExt, JsValue>;
            }

            let array = Uint8Array::new_with_length(image.rgba.len() as u32);
            array.copy_from(&image.rgba);
            let array = Uint8ClampedArray::new(&array);
            ImageDataExt::new(array, image.width as u32)
                .map(JsValue::from)
                .map(ImageData::unchecked_from_js)
                .unwrap()
        };
        #[cfg(not(target_feature = "atomics"))]
        let image_data = ImageData::new_with_u8_clamped_array(
            wasm_bindgen::Clamped(&image.rgba),
            image.width as u32,
        )
        .unwrap();

        let mut options = ImageBitmapOptions::new();
        options.premultiply_alpha(PremultiplyAlpha::None);
        let bitmap = JsFuture::from(
            window
                .create_image_bitmap_with_image_data_and_image_bitmap_options(&image_data, &options)
                .unwrap(),
        );

        let state = Rc::new(RefCell::new(Some(Self::Loading {
            style,
            cursor_visible,
            previous,
            hotspot_x: image.hotspot_x,
            hotspot_y: image.hotspot_y,
        })));

        wasm_bindgen_futures::spawn_local({
            let weak = Rc::downgrade(&state);
            let CursorImage { width, height, .. } = *image;
            async move {
                if weak.strong_count() == 0 {
                    return;
                }

                let bitmap: ImageBitmap = bitmap.await.unwrap().unchecked_into();

                if weak.strong_count() == 0 {
                    return;
                }

                let canvas: HtmlCanvasElement =
                    document.create_element("canvas").unwrap().unchecked_into();
                #[allow(clippy::disallowed_methods)]
                canvas.set_width(width as u32);
                #[allow(clippy::disallowed_methods)]
                canvas.set_height(height as u32);

                let context: ImageBitmapRenderingContext = canvas
                    .get_context("bitmaprenderer")
                    .unwrap()
                    .unwrap()
                    .unchecked_into();
                context.transfer_from_image_bitmap(&bitmap);

                thread_local! {
                    static CURRENT_STATE: RefCell<Option<Weak<RefCell<Option<CursorImageState>>>>> = RefCell::new(None);
                    // `HTMLCanvasElement.toBlob()` can't be interrupted. So we have to use a
                    // `Closure` that doesn't need to be garbage-collected.
                    static CALLBACK: Closure<dyn Fn(Option<Blob>)> = Closure::new(|blob| {
                        CURRENT_STATE.with(|weak| {
                            let Some(state) = weak.borrow_mut().take().and_then(|weak| weak.upgrade()) else {
                                return;
                            };
                            let mut state = state.borrow_mut();
                            // Extract old state.
                            let CursorImageState::Loading { style, cursor_visible, previous, hotspot_x, hotspot_y, .. } = state.take().unwrap() else {
                                unreachable!("found invalid state")
                            };

                            let Some(blob) = blob else {
                                *state = Some(CursorImageState::Failed(previous));
                                return;
                            };
                            let data_url = Url::create_object_url_with_blob(&blob).unwrap();

                            let value = previous.style_with_url(&data_url, hotspot_x, hotspot_y);

                            if cursor_visible.get() {
                                style.set("cursor", &value);
                            }

                            *state = Some(
                                CursorImageState::Ready {
                                    style: value,
                                    image: WebCursorImage{ data_url, hotspot_x, hotspot_y },
                                    previous,
                                });
                        });
                    });
                }

                CURRENT_STATE.with(|state| *state.borrow_mut() = Some(weak));
                CALLBACK
                    .with(|callback| canvas.to_blob(callback.as_ref().unchecked_ref()).unwrap());
            }
        });

        state
    }
}

#[derive(Debug)]
pub struct WebCursorImage {
    data_url: String,
    hotspot_x: u16,
    hotspot_y: u16,
}

impl Drop for WebCursorImage {
    fn drop(&mut self) {
        Url::revoke_object_url(&self.data_url).unwrap();
    }
}
