mod canvas;
mod event;
mod event_handle;
mod media_query_handle;
mod scaling;
mod timeout;

pub use self::canvas::Canvas;
pub use self::scaling::ScaleChangeDetector;
pub use self::timeout::{AnimationFrameRequest, Timeout};

use crate::dpi::{LogicalSize, Size};
use crate::platform::web::WindowExtWebSys;
use crate::window::Window;
use wasm_bindgen::prelude::*;
use web_sys::{window, BeforeUnloadEvent, Element, HtmlCanvasElement};

pub fn throw(msg: &str) {
    wasm_bindgen::throw_str(msg);
}

pub fn exit_fullscreen() {
    let window = web_sys::window().expect("Failed to obtain window");
    let document = window.document().expect("Failed to obtain document");

    document.exit_fullscreen();
}

pub struct UnloadEventHandle {
    _listener: event_handle::EventListenerHandle<dyn FnMut(BeforeUnloadEvent)>,
}

pub fn on_unload(mut handler: impl FnMut() + 'static) -> UnloadEventHandle {
    let window = web_sys::window().expect("Failed to obtain window");

    let closure = Closure::wrap(
        Box::new(move |_: BeforeUnloadEvent| handler()) as Box<dyn FnMut(BeforeUnloadEvent)>
    );

    let listener = event_handle::EventListenerHandle::new(&window, "beforeunload", closure);
    UnloadEventHandle {
        _listener: listener,
    }
}

impl WindowExtWebSys for Window {
    fn canvas(&self) -> HtmlCanvasElement {
        self.window.canvas().raw().clone()
    }

    fn is_dark_mode(&self) -> bool {
        let window = web_sys::window().expect("Failed to obtain window");

        window
            .match_media("(prefers-color-scheme: dark)")
            .ok()
            .flatten()
            .map(|media| media.matches())
            .unwrap_or(false)
    }
}

pub fn scale_factor() -> f64 {
    let window = web_sys::window().expect("Failed to obtain window");
    window.device_pixel_ratio()
}

/// Gets the size of the content box of `element` based on CSS.
///
/// Returns `None` if the element isn't in the DOM.
pub fn inner_size(raw: &HtmlCanvasElement) -> Option<LogicalSize<f64>> {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    if !document.contains(Some(raw)) {
        return None;
    }

    // Use `getBoundingClientRect` instead of the width and height properties because it doesn't round to the nearest integer.
    let rect = raw.get_bounding_client_rect();
    let style = window
        .get_computed_style(raw)
        .unwrap()
        .expect("`getComputedStyle` returned `None`");

    let display_none = style.get_property_value("display").unwrap() == "none";

    let prop = |name| -> f64 {
        let value = style.get_property_value(name).unwrap();
        if display_none && name.starts_with("padding") {
            // When `display` is `none`, the value returned for padding isn't
            // guaranteed to be in `px` (it's left as a percentage if the
            // property is specified as such, when normally it's resolved to
            // `px`).
            // Note: that's also true when `display` is `contents`, but for
            // `<canvas>` that gets resolved to `display: none` and can never
            // happen.
            // So, return 0, since getting the size right isn't particularly
            // important for an invisible element.
            return 0.0;
        }
        // Remove the `px` from the end of the value and parse it.
        value
            .strip_suffix("px")
            .expect("border and padding should always be in units of `px`")
            .parse()
            .unwrap()
    };

    Some(LogicalSize {
        width: rect.width()
            - prop("border-left-width")
            - prop("border-right-width")
            - prop("padding-left")
            - prop("padding-right"),
        height: rect.height()
            - prop("border-top-width")
            - prop("border-bottom-width")
            - prop("padding-top")
            - prop("padding-bottom"),
    })
}

pub fn set_inner_size(raw: &HtmlCanvasElement, size: Size) {
    let scale_factor = scale_factor();

    let mut logical_size = size.to_logical::<f64>(scale_factor);

    if cfg!(not(feature = "css-size")) {
        let physical_size = size.to_physical(scale_factor);
        raw.set_width(physical_size.width);
        raw.set_height(physical_size.height);
    }

    let window = web_sys::window().unwrap();
    let style = window
        .get_computed_style(raw)
        // This can't fail according to the spec; I don't know why web-sys marks it as throwing and having an optional result.
        .expect("`getComputedStyle` failed")
        .expect("`getComputedStyle` returned `None`");

    // This also can't fail according to the spec.
    if style.get_property_value("box-sizing").unwrap() == "border-box" {
        let prop = |name| -> f64 {
            let value = style.get_property_value(name).unwrap();
            // Cut off the last two characters to remove the `px` from the end.
            value[..value.len() - 2].parse().unwrap()
        };

        logical_size.width += prop("border-left-width")
            + prop("border-right-width")
            + prop("padding-left")
            + prop("padding-right");
        logical_size.height += prop("border-top-width")
            + prop("border-bottom-width")
            + prop("padding-top")
            + prop("padding-bottom");
    }

    set_canvas_style_property(raw, "width", &format!("{}px", logical_size.width));
    set_canvas_style_property(raw, "height", &format!("{}px", logical_size.height));
}

pub fn set_canvas_style_property(raw: &HtmlCanvasElement, property: &str, value: &str) {
    let style = raw.style();
    style
        .set_property(property, value)
        .unwrap_or_else(|err| panic!("error: {:?}\nFailed to set {}", err, property))
}

pub fn is_fullscreen(canvas: &HtmlCanvasElement) -> bool {
    let window = window().expect("Failed to obtain window");
    let document = window.document().expect("Failed to obtain document");

    match document.fullscreen_element() {
        Some(elem) => {
            let raw: Element = canvas.clone().into();
            raw == elem
        }
        None => false,
    }
}

// A slight hack to get at the prototype of `ResizeObserverEntry`, so that we can check for `device-pixel-content-box` support.
#[cfg(feature = "css-size")]
mod prototype {
    use js_sys::Object;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen]
        pub type ResizeObserverEntry;

        #[wasm_bindgen(static_method_of = ResizeObserverEntry, getter)]
        pub fn prototype() -> Object;
    }
}

#[cfg(feature = "css-size")]
pub fn supports_device_pixel_content_size() -> bool {
    use js_sys::Object;

    let proto = prototype::ResizeObserverEntry::prototype();
    let desc = Object::get_own_property_descriptor(
        &proto,
        &JsValue::from_str("devicePixelContentBoxSize"),
    );
    !desc.is_undefined()
}

pub type RawCanvasType = HtmlCanvasElement;
