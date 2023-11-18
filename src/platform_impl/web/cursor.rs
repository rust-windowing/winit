use std::{
    cell::RefCell,
    mem,
    ops::{Deref, DerefMut},
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
        hotspot_x: u32,
        hotspot_y: u32,
    },
}

impl WebCustomCursor {
    pub fn from_rgba(
        rgba: Vec<u8>,
        width: u32,
        height: u32,
        hotspot_x: u32,
        hotspot_y: u32,
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
    ) -> SelectedCursor {
        match self {
            WebCustomCursor::Image(image) => SelectedCursor::Image(CursorImageState::from_image(
                window,
                document.clone(),
                style.clone(),
                image,
                previous,
            )),
            WebCustomCursor::Url {
                url,
                hotspot_x,
                hotspot_y,
            } => {
                let value = format!("url({}) {} {}, auto", url, hotspot_x, hotspot_y);
                style.set("cursor", &value);
                SelectedCursor::Url(value)
            }
        }
    }
}

#[derive(Debug)]
pub enum SelectedCursor {
    Named(CursorIcon),
    Url(String),
    Image(Rc<RefCell<CursorImageState>>),
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
            SelectedCursor::Url(url) => url,
            SelectedCursor::Image(image) => {
                let image = image.borrow();
                let value = match image.deref() {
                    CursorImageState::Loading { previous, .. } => previous.style(),
                    CursorImageState::Ready(WebCursorImage { style, .. }) => style,
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
    Url(String),
    Image(WebCursorImage),
}

impl Previous {
    fn style(&self) -> &str {
        match self {
            Previous::Named(icon) => icon.name(),
            Previous::Url(url) => url,
            Previous::Image(WebCursorImage { style, .. }) => style,
        }
    }
}

impl From<SelectedCursor> for Previous {
    fn from(value: SelectedCursor) -> Self {
        match value {
            SelectedCursor::Named(icon) => Self::Named(icon),
            SelectedCursor::Url(url) => Self::Url(url),
            SelectedCursor::Image(image) => match Rc::into_inner(image).unwrap().into_inner() {
                CursorImageState::Loading { previous, .. } => previous,
                CursorImageState::Ready(internal) => Self::Image(internal),
            },
        }
    }
}

#[derive(Debug)]
pub enum CursorImageState {
    Loading {
        style: Style,
        previous: Previous,
        hotspot_x: u32,
        hotspot_y: u32,
    },
    Ready(WebCursorImage),
}

impl CursorImageState {
    fn from_image(
        window: &Window,
        document: Document,
        style: Style,
        image: &CursorImage,
        previous: SelectedCursor,
    ) -> Rc<RefCell<Self>> {
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
            ImageDataExt::new(array, image.width)
                .map(JsValue::from)
                .map(ImageData::unchecked_from_js)
                .unwrap()
        };
        #[cfg(not(target_feature = "atomics"))]
        let image_data =
            ImageData::new_with_u8_clamped_array(wasm_bindgen::Clamped(&image.rgba), image.width)
                .unwrap();

        let mut options = ImageBitmapOptions::new();
        options.premultiply_alpha(PremultiplyAlpha::None);
        let bitmap = JsFuture::from(
            window
                .create_image_bitmap_with_image_data_and_image_bitmap_options(&image_data, &options)
                .unwrap(),
        );

        let state = Rc::new(RefCell::new(Self::Loading {
            style,
            previous: previous.into(),
            hotspot_x: image.hotspot_x,
            hotspot_y: image.hotspot_y,
        }));

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
                canvas.set_width(width);
                #[allow(clippy::disallowed_methods)]
                canvas.set_height(height);

                let context: ImageBitmapRenderingContext = canvas
                    .get_context("bitmaprenderer")
                    .unwrap()
                    .unwrap()
                    .unchecked_into();
                context.transfer_from_image_bitmap(&bitmap);

                thread_local! {
                    static CURRENT_STATE: RefCell<Option<Weak<RefCell<CursorImageState>>>> = RefCell::new(None);
                    // `HTMLCanvasElement.toBlob()` can't be interrupted. So we have to use a
                    // `Closure` that doesn't need to be garbage-collected.
                    static CALLBACK: Closure<dyn Fn(Option<Blob>)> = Closure::new(|blob| {
                        CURRENT_STATE.with(|weak| {
                            let Some(blob) = blob else {
                                return;
                            };
                            let Some(state) = weak.borrow_mut().take().and_then(|weak| weak.upgrade()) else {
                                return;
                            };
                            let mut state = state.borrow_mut();
                            // Extract the hotspots.
                            let CursorImageState::Loading { hotspot_x, hotspot_y, .. } = *state else {
                                unreachable!("found invalid state")
                            };

                            let data_url = Url::create_object_url_with_blob(&blob).unwrap();
                            // Extract `Style`, which we couldn't do earlier without cloning it.
                            let CursorImageState::Loading { style, .. } = mem::replace(state.deref_mut(), CursorImageState::Ready(WebCursorImage {
                                style: format!("url({}) {} {}, auto", data_url, hotspot_x, hotspot_y),
                                data_url,
                            })) else {
                                unreachable!("found invalid state")
                            };
                            // Extract the `cursor` property value, which we couldn't cache earlier without cloning.
                            let CursorImageState::Ready(WebCursorImage { style: value, .. }) = state.deref() else {
                                unreachable!("found invalid state")
                            };
                            style.set("cursor", value);
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
    style: String,
    data_url: String,
}

impl Drop for WebCursorImage {
    fn drop(&mut self) {
        Url::revoke_object_url(&self.data_url).unwrap();
    }
}
