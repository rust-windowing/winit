use js_sys::{Array, Object};
use once_cell::unsync::Lazy;
use wasm_bindgen::prelude::{wasm_bindgen, Closure};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
    HtmlCanvasElement, ResizeObserver, ResizeObserverBoxOptions, ResizeObserverEntry,
    ResizeObserverOptions, ResizeObserverSize, Window,
};

use crate::dpi::{LogicalSize, PhysicalSize};

use super::super::backend;

pub struct ResizeHandle {
    observer: ResizeObserver,
    _closure: Closure<dyn FnMut(Array)>,
}

impl ResizeHandle {
    pub fn new<F>(window: Window, canvas: HtmlCanvasElement, mut listener: F) -> Self
    where
        F: 'static + FnMut(PhysicalSize<u32>),
    {
        let closure = Closure::new({
            let canvas = canvas.clone();
            move |entries: Array| {
                let size = Self::process_entry(&window, &canvas, entries);

                listener(size)
            }
        });
        let observer = ResizeObserver::new(closure.as_ref().unchecked_ref())
            .expect("Failed to create `ResizeObserver`");

        // Safari doesn't support `devicePixelContentBoxSize`
        if has_device_pixel_support() {
            observer.observe_with_options(
                &canvas,
                ResizeObserverOptions::new().box_(ResizeObserverBoxOptions::DevicePixelContentBox),
            );
        } else {
            observer.observe(&canvas);
        }

        Self {
            observer,
            _closure: closure,
        }
    }

    fn process_entry(
        window: &Window,
        canvas: &HtmlCanvasElement,
        entries: Array,
    ) -> PhysicalSize<u32> {
        debug_assert_eq!(entries.length(), 1, "expected exactly one entry");
        let entry = entries.get(0);
        debug_assert!(entry.has_type::<ResizeObserverEntry>());
        let entry: ResizeObserverEntry = entry.unchecked_into();

        // Safari doesn't support `devicePixelContentBoxSize`
        if !has_device_pixel_support() {
            let rect = entry.content_rect();

            return LogicalSize::new(rect.width(), rect.height())
                .to_physical(backend::scale_factor(window));
        }

        let entries = entry.device_pixel_content_box_size();
        debug_assert_eq!(
            entries.length(),
            1,
            "a canvas can't be split into multiple fragments"
        );
        let entry = entries.get(0);
        debug_assert!(entry.has_type::<ResizeObserverSize>());
        let entry: ResizeObserverSize = entry.unchecked_into();

        let style = window
            .get_computed_style(canvas)
            .expect("Failed to get computed style of canvas")
            // this can only be empty if we provided an invalid `pseudoElt`
            .expect("`getComputedStyle` can not be empty");

        let writing_mode = style
            .get_property_value("writing-mode")
            .expect("`wirting-mode` is a valid CSS property");

        // means the canvas is not inserted into the DOM
        if writing_mode.is_empty() {
            debug_assert_eq!(entry.inline_size(), 0.);
            debug_assert_eq!(entry.block_size(), 0.);

            return PhysicalSize::new(0, 0);
        }

        let horizontal = match writing_mode.as_str() {
            _ if writing_mode.starts_with("horizontal") => true,
            _ if writing_mode.starts_with("vertical") | writing_mode.starts_with("sideways") => {
                false
            }
            // deprecated values
            "lr" | "lr-tb" | "rl" => true,
            "tb" | "tb-lr" | "tb-rl" => false,
            _ => {
                warn!("unrecognized `writing-mode`, assuming horizontal");
                true
            }
        };

        if horizontal {
            PhysicalSize::new(entry.inline_size() as u32, entry.block_size() as u32)
        } else {
            PhysicalSize::new(entry.block_size() as u32, entry.inline_size() as u32)
        }
    }
}

impl Drop for ResizeHandle {
    fn drop(&mut self) {
        self.observer.disconnect();
    }
}

fn has_device_pixel_support() -> bool {
    thread_local! {
        static DEVICE_PIXEL_SUPPORT: Lazy<bool> = Lazy::new(|| {
            #[wasm_bindgen]
            extern "C" {
                type ResizeObserverEntryExt;

                #[wasm_bindgen(js_class = ResizeObserverEntry, static_method_of = ResizeObserverEntryExt, getter)]
                fn prototype() -> Object;
            }

            let prototype = ResizeObserverEntryExt::prototype();
            let descriptor = Object::get_own_property_descriptor(
                &prototype,
                &JsValue::from_str("devicePixelContentBoxSize"),
            );
            !descriptor.is_undefined()
        });
    }

    DEVICE_PIXEL_SUPPORT.with(|support| **support)
}
