use crate::cursor::{BadImage, CursorImage};
use cursor_icon::CursorIcon;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, Document, HtmlCanvasElement, ImageData, Url};

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
}

#[derive(Debug)]
pub enum SelectedCursor {
    Named(CursorIcon),
    Custom(CustomCursorInternal),
}

impl Default for SelectedCursor {
    fn default() -> Self {
        Self::Named(Default::default())
    }
}

#[derive(Debug)]
pub struct CustomCursorInternal {
    style: String,
    data_url: Option<String>,
}

impl CustomCursorInternal {
    pub fn new(document: &Document, cursor: &WebCustomCursor) -> Self {
        match cursor {
            WebCustomCursor::Image(image) => Self::from_image(document, image),
            WebCustomCursor::Url {
                url,
                hotspot_x,
                hotspot_y,
            } => Self {
                style: format!("url({}) {} {}, auto", url, hotspot_x, hotspot_y),
                data_url: None,
            },
        }
    }

    fn from_image(document: &Document, image: &CursorImage) -> Self {
        let cursor_icon_canvas: HtmlCanvasElement =
            document.create_element("canvas").unwrap().unchecked_into();

        #[allow(clippy::disallowed_methods)]
        cursor_icon_canvas.set_width(image.width);
        #[allow(clippy::disallowed_methods)]
        cursor_icon_canvas.set_height(image.height);

        let context: CanvasRenderingContext2d = cursor_icon_canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .unchecked_into();

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

        context.put_image_data(&image_data, 0.0, 0.0).unwrap();

        let data_url = cursor_icon_canvas.to_data_url().unwrap();

        Self {
            style: format!(
                "url({}) {} {}, auto",
                data_url, image.hotspot_x, image.hotspot_y
            ),
            data_url: Some(data_url),
        }
    }

    pub fn style(&self) -> &str {
        &self.style
    }
}

impl Drop for CustomCursorInternal {
    fn drop(&mut self) {
        if let Some(data_url) = &self.data_url {
            Url::revoke_object_url(data_url).unwrap();
        };
    }
}
