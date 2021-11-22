use js_sys::Array;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;
use web_sys::ResizeObserver;
use web_sys::ResizeObserverBoxOptions;
use web_sys::ResizeObserverEntry;
use web_sys::ResizeObserverOptions;
use web_sys::ResizeObserverSize;

use crate::dpi::LogicalSize;
use crate::dpi::PhysicalSize;
use crate::platform_impl::platform::backend;
use crate::platform_impl::platform::backend::ScaleChangeDetector;
use crate::window::WindowId;

use super::runner::Shared;

fn process_entry(entry: &ResizeObserverEntry) -> (WindowId, HtmlCanvasElement, PhysicalSize<u32>) {
    let canvas: HtmlCanvasElement = entry.target().dyn_into().unwrap();

    let id = WindowId(super::window::Id(
        canvas
            .get_attribute("data-raw-handle")
            .expect("Canvas was missing `data-raw-handle` attribute")
            .parse()
            .expect("Canvas had invalid `data-raw-handle` attribute"),
    ));

    let size = if entry.device_pixel_content_box_size().is_undefined() {
        // Safari doesn't support `devicePixelContentBoxSize` yet (nor `contentBoxSize`), so fall back to `contentRect` and use the scale factor to convert.
        let rect = entry.content_rect();

        LogicalSize::new(rect.width(), rect.height()).to_physical(super::backend::scale_factor())
    } else {
        // TODO: what exactly would cause there to be multiple of these?
        let size: ResizeObserverSize = entry
            .device_pixel_content_box_size()
            .get(0)
            .dyn_into()
            .expect(
                "`ResizeObserverEntry.devicePixelContentBoxSize` was not a `ResizeObserverSize`",
            );

        let window = web_sys::window().unwrap();
        let style = window
            .get_computed_style(&canvas)
            .expect("`getComputedStyle` failed")
            .expect("`getComputedStyle` returned `None`");

        match style.get_property_value("writing-mode").unwrap().as_str() {
            "vertical-lr" | "vertical-rl" | "sideways-lr" | "sideways-rl" | "tb" | "tb-lr"
            | "tb-rl" => {
                // The text is flowing vertically, so `inline_size` is height and `block_size` is width.
                PhysicalSize {
                    width: size.block_size() as u32,
                    height: size.inline_size() as u32,
                }
            }
            // If it isn't a known value, default to horizontal,
            // since it's probably a browser which doesn't support this or something.
            _ => PhysicalSize {
                width: size.inline_size() as u32,
                height: size.block_size() as u32,
            },
        }
    };

    (id, canvas, size)
}

pub enum ResizeState {
    /// Used on platforms with support for `device-pixel-content-box`.
    ///
    /// `physical_observer`, a `ResizeObserver` configured to watch `device-pixel-content-box`, is used most of the time.
    /// However, sometimes when the scale factor has changed, only the logical sizes of the canvases will have changed,
    /// and we'll fall back on `logical_observer`.
    WithPhysicalObserver {
        _logical_closure: Closure<dyn FnMut()>,
        _physical_closure: Closure<dyn FnMut(Array)>,
        logical_observer: ResizeObserver,
        physical_observer: ResizeObserver,
    },
    /// Used on platforms without support for `device-pixel-content-box`.
    ///
    /// `observer`, a `ResizeObserver` configured to watch the logical size, is mostly used.
    /// However, sometimes when the scale factor changes, only the physical sizes of the canvases change,
    /// in which case `scale_change_detector` is used.
    NoPhysicalObserver {
        scale_change_detector: ScaleChangeDetector,
        closure: Closure<dyn FnMut(Array)>,
        observer: ResizeObserver,
    },
}

impl ResizeState {
    pub fn new<T>(runner: Shared<T>) -> Self {
        if backend::supports_device_pixel_content_size() {
            let physical_closure = {
                let runner = runner.clone();
                Closure::wrap(Box::new(move |entries: Array| {
                    let resizes: Vec<_> = entries
                        .iter()
                        .map(|entry| {
                            let entry: ResizeObserverEntry = entry.dyn_into().expect("`ResizeObserver` callback not called with array of `ResizeObserverEntry`");

                            process_entry(&entry)
                        })
                        .collect();

                    runner.handle_resizes(resizes);
                }) as Box<dyn FnMut(_)>)
            };

            let logical_closure = Closure::wrap(Box::new(move || {
                if runner.scale_factor_changed() {
                    // If the scale factor is still incorrect, the physical `ResizeObserver` must not have run.
                    // Just call this with an empty `Vec`, since it'll then automatically resize everything to its existing size,
                    // which is correct because none of them must have changed for the physical `ResizeObserver` not to have run.
                    runner.handle_resizes(vec![]);
                }
            }) as Box<dyn FnMut()>);

            // Create the physical `ResizeObserver` first, because that'll make it fire first.
            // It will handle everything most of the time, and the logical `ResizeObserver` will only do anything if the physical one hasn't run.
            let physical_observer =
                ResizeObserver::new(physical_closure.as_ref().unchecked_ref()).unwrap();
            let logical_observer =
                ResizeObserver::new(logical_closure.as_ref().unchecked_ref()).unwrap();

            Self::WithPhysicalObserver {
                _logical_closure: logical_closure,
                _physical_closure: physical_closure,
                logical_observer,
                physical_observer,
            }
        } else {
            let scale_change_detector = {
                let runner = runner.clone();
                ScaleChangeDetector::new(move || runner.handle_scale_changed(true))
            };

            let closure = Closure::wrap(Box::new(move |entries: Array| {
                if runner.scale_factor_changed() {
                    runner.handle_scale_changed(false);
                } else {
                    let resizes = entries.iter().map(|entry| {
                        let entry: ResizeObserverEntry = entry.dyn_into().expect("`ResizeObserver` callback not called with array of `ResizeObserverEntry`");

                        process_entry(&entry)
                    }).collect();

                    runner.handle_resizes(resizes)
                }
            }) as Box<dyn FnMut(_)>);

            let observer = ResizeObserver::new(closure.as_ref().unchecked_ref()).unwrap();

            Self::NoPhysicalObserver {
                scale_change_detector,
                closure,
                observer,
            }
        }
    }

    pub fn observe(&self, canvas: &HtmlCanvasElement) {
        match self {
            Self::WithPhysicalObserver {
                logical_observer,
                physical_observer,
                ..
            } => {
                logical_observer.observe(canvas);
                physical_observer.observe_with_options(
                    canvas,
                    ResizeObserverOptions::new()
                        .box_(ResizeObserverBoxOptions::DevicePixelContentBox),
                );
            }
            Self::NoPhysicalObserver { observer, .. } => observer.observe(canvas),
        }
    }

    pub fn unobserve(&self, canvas: &HtmlCanvasElement) {
        match self {
            Self::WithPhysicalObserver {
                logical_observer,
                physical_observer,
                ..
            } => {
                logical_observer.unobserve(canvas);
                physical_observer.unobserve(canvas);
            }
            Self::NoPhysicalObserver { observer, .. } => observer.unobserve(canvas),
        }
    }
}

impl Drop for ResizeState {
    fn drop(&mut self) {
        match self {
            Self::WithPhysicalObserver {
                logical_observer,
                physical_observer,
                ..
            } => {
                logical_observer.disconnect();
                physical_observer.disconnect();
            }
            Self::NoPhysicalObserver { observer, .. } => observer.disconnect(),
        }
    }
}
