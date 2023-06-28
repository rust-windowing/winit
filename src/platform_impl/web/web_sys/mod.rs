mod canvas;
pub mod event;
mod event_handle;
mod media_query_handle;
mod pointer;
mod resize_scaling;
mod timeout;

pub use self::canvas::Canvas;
pub use self::event::ButtonsState;
pub use self::event_handle::EventListenerHandle;
pub use self::resize_scaling::ResizeScaleHandle;
pub use self::timeout::{IdleCallback, Timeout};

use crate::dpi::LogicalSize;
use crate::platform::web::WindowExtWebSys;
use crate::window::Window;
use wasm_bindgen::closure::Closure;
use web_sys::{CssStyleDeclaration, Element, HtmlCanvasElement, PageTransitionEvent};

pub fn throw(msg: &str) {
    wasm_bindgen::throw_str(msg);
}

pub fn exit_fullscreen(window: &web_sys::Window) {
    let document = window.document().expect("Failed to obtain document");

    document.exit_fullscreen();
}

pub struct PageTransitionEventHandle {
    _show_listener: event_handle::EventListenerHandle<dyn FnMut(PageTransitionEvent)>,
    _hide_listener: event_handle::EventListenerHandle<dyn FnMut(PageTransitionEvent)>,
}

pub fn on_page_transition(
    window: &web_sys::Window,
    show_handler: impl FnMut(PageTransitionEvent) + 'static,
    hide_handler: impl FnMut(PageTransitionEvent) + 'static,
) -> PageTransitionEventHandle {
    let show_closure = Closure::new(show_handler);
    let hide_closure = Closure::new(hide_handler);

    let show_listener = event_handle::EventListenerHandle::new(window, "pageshow", show_closure);
    let hide_listener = event_handle::EventListenerHandle::new(window, "pagehide", hide_closure);
    PageTransitionEventHandle {
        _show_listener: show_listener,
        _hide_listener: hide_listener,
    }
}

impl WindowExtWebSys for Window {
    fn canvas(&self) -> Option<HtmlCanvasElement> {
        self.window.canvas()
    }

    fn is_dark_mode(&self) -> bool {
        self.window
            .inner
            .queue(|inner| is_dark_mode(&inner.window).unwrap_or(false))
    }
}

pub fn scale_factor(window: &web_sys::Window) -> f64 {
    window.device_pixel_ratio()
}

pub fn set_canvas_size(
    window: &web_sys::Window,
    raw: &HtmlCanvasElement,
    mut new_size: LogicalSize<f64>,
) {
    let document = window.document().expect("Failed to obtain document");

    let style = window
        .get_computed_style(raw)
        .expect("Failed to obtain computed style")
        // this can't fail: we aren't using a pseudo-element
        .expect("Invalid pseudo-element");

    if !document.contains(Some(raw)) || style.get_property_value("display").unwrap() == "none" {
        return;
    }

    if style.get_property_value("box-sizing").unwrap() == "border-box" {
        new_size.width += style_size_property(&style, "border-left-width")
            + style_size_property(&style, "border-right-width")
            + style_size_property(&style, "padding-left")
            + style_size_property(&style, "padding-right");
        new_size.height += style_size_property(&style, "border-top-width")
            + style_size_property(&style, "border-bottom-width")
            + style_size_property(&style, "padding-top")
            + style_size_property(&style, "padding-bottom");
    }

    set_canvas_style_property(raw, "width", &format!("{}px", new_size.width));
    set_canvas_style_property(raw, "height", &format!("{}px", new_size.height));
}

/// This function will panic if the element is not inserted in the DOM
/// or is not a CSS property that represents a size in pixel.
pub fn style_size_property(style: &CssStyleDeclaration, property: &str) -> f64 {
    let prop = style
        .get_property_value(property)
        .expect("Found invalid property");
    prop.strip_suffix("px")
        .expect("Element was not inserted into the DOM or is not a size in pixel")
        .parse()
        .expect("CSS property is not a size in pixel")
}

pub fn set_canvas_style_property(raw: &HtmlCanvasElement, property: &str, value: &str) {
    let style = raw.style();
    style
        .set_property(property, value)
        .unwrap_or_else(|err| panic!("error: {err:?}\nFailed to set {property}"))
}

pub fn is_fullscreen(window: &web_sys::Window, canvas: &HtmlCanvasElement) -> bool {
    let document = window.document().expect("Failed to obtain document");

    match document.fullscreen_element() {
        Some(elem) => {
            let raw: Element = canvas.clone().into();
            raw == elem
        }
        None => false,
    }
}

pub fn is_dark_mode(window: &web_sys::Window) -> Option<bool> {
    window
        .match_media("(prefers-color-scheme: dark)")
        .ok()
        .flatten()
        .map(|media| media.matches())
}

pub type RawCanvasType = HtmlCanvasElement;
