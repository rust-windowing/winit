use js_sys::{Array, Object};
use tracing::warn;
use wasm_bindgen::prelude::{wasm_bindgen, Closure};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
    Document, HtmlCanvasElement, MediaQueryList, ResizeObserver, ResizeObserverBoxOptions,
    ResizeObserverEntry, ResizeObserverOptions, ResizeObserverSize, Window,
};

use crate::dpi::{LogicalSize, PhysicalSize};

use super::super::backend;
use super::canvas::Style;
use super::media_query_handle::MediaQueryListHandle;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct ResizeScaleHandle(Rc<ResizeScaleInternal>);

impl ResizeScaleHandle {
    pub(crate) fn new<S, R>(
        window: Window,
        document: Document,
        canvas: HtmlCanvasElement,
        style: Style,
        scale_handler: S,
        resize_handler: R,
    ) -> Self
    where
        S: 'static + Fn(PhysicalSize<u32>, f64),
        R: 'static + Fn(PhysicalSize<u32>),
    {
        Self(ResizeScaleInternal::new(
            window,
            document,
            canvas,
            style,
            scale_handler,
            resize_handler,
        ))
    }

    pub(crate) fn notify_resize(&self) {
        self.0.notify()
    }
}

/// This is a helper type to help manage the `MediaQueryList` used for detecting
/// changes of the `devicePixelRatio`.
struct ResizeScaleInternal {
    window: Window,
    document: Document,
    canvas: HtmlCanvasElement,
    style: Style,
    mql: RefCell<MediaQueryListHandle>,
    observer: ResizeObserver,
    _observer_closure: Closure<dyn FnMut(Array, ResizeObserver)>,
    scale_handler: Box<dyn Fn(PhysicalSize<u32>, f64)>,
    resize_handler: Box<dyn Fn(PhysicalSize<u32>)>,
    notify_scale: Cell<bool>,
}

impl ResizeScaleInternal {
    fn new<S, R>(
        window: Window,
        document: Document,
        canvas: HtmlCanvasElement,
        style: Style,
        scale_handler: S,
        resize_handler: R,
    ) -> Rc<Self>
    where
        S: 'static + Fn(PhysicalSize<u32>, f64),
        R: 'static + Fn(PhysicalSize<u32>),
    {
        Rc::<ResizeScaleInternal>::new_cyclic(|weak_self| {
            let mql = Self::create_mql(&window, {
                let weak_self = weak_self.clone();
                move |mql| {
                    if let Some(rc_self) = weak_self.upgrade() {
                        Self::handle_scale(rc_self, mql);
                    }
                }
            });

            let weak_self = weak_self.clone();
            let observer_closure = Closure::new(move |entries: Array, _| {
                if let Some(this) = weak_self.upgrade() {
                    let size = this.process_entry(entries);

                    if this.notify_scale.replace(false) {
                        let scale = backend::scale_factor(&this.window);
                        (this.scale_handler)(size, scale)
                    } else {
                        (this.resize_handler)(size)
                    }
                }
            });
            let observer = Self::create_observer(&canvas, observer_closure.as_ref());

            Self {
                window,
                document,
                canvas,
                style,
                mql: RefCell::new(mql),
                observer,
                _observer_closure: observer_closure,
                scale_handler: Box::new(scale_handler),
                resize_handler: Box::new(resize_handler),
                notify_scale: Cell::new(false),
            }
        })
    }

    fn create_mql<F>(window: &Window, closure: F) -> MediaQueryListHandle
    where
        F: 'static + FnMut(&MediaQueryList),
    {
        let current_scale = super::scale_factor(window);
        // TODO: Remove `-webkit-device-pixel-ratio`. Requires Safari v16.
        let media_query = format!(
            "(resolution: {current_scale}dppx),
             (-webkit-device-pixel-ratio: {current_scale})",
        );
        let mql = MediaQueryListHandle::new(window, &media_query, closure);
        debug_assert!(
            mql.mql().matches(),
            "created media query doesn't match, {current_scale} != {}",
            super::scale_factor(window)
        );
        mql
    }

    fn create_observer(canvas: &HtmlCanvasElement, closure: &JsValue) -> ResizeObserver {
        let observer = ResizeObserver::new(closure.as_ref().unchecked_ref())
            .expect("Failed to create `ResizeObserver`");

        // Safari doesn't support `devicePixelContentBoxSize`
        if has_device_pixel_support() {
            let options = ResizeObserverOptions::new();
            options.set_box(ResizeObserverBoxOptions::DevicePixelContentBox);
            observer.observe_with_options(canvas, &options);
        } else {
            observer.observe(canvas);
        }

        observer
    }

    fn notify(&self) {
        if !self.document.contains(Some(&self.canvas)) || self.style.get("display") == "none" {
            let size = PhysicalSize::new(0, 0);

            if self.notify_scale.replace(false) {
                let scale = backend::scale_factor(&self.window);
                (self.scale_handler)(size, scale)
            } else {
                (self.resize_handler)(size)
            }

            return;
        }

        // Safari doesn't support `devicePixelContentBoxSize`
        if has_device_pixel_support() {
            self.observer.unobserve(&self.canvas);
            self.observer.observe(&self.canvas);

            return;
        }

        let mut size = LogicalSize::new(
            backend::style_size_property(&self.style, "width"),
            backend::style_size_property(&self.style, "height"),
        );

        if self.style.get("box-sizing") == "border-box" {
            size.width -= backend::style_size_property(&self.style, "border-left-width")
                + backend::style_size_property(&self.style, "border-right-width")
                + backend::style_size_property(&self.style, "padding-left")
                + backend::style_size_property(&self.style, "padding-right");
            size.height -= backend::style_size_property(&self.style, "border-top-width")
                + backend::style_size_property(&self.style, "border-bottom-width")
                + backend::style_size_property(&self.style, "padding-top")
                + backend::style_size_property(&self.style, "padding-bottom");
        }

        let size = size.to_physical(backend::scale_factor(&self.window));

        if self.notify_scale.replace(false) {
            let scale = backend::scale_factor(&self.window);
            (self.scale_handler)(size, scale)
        } else {
            (self.resize_handler)(size)
        }
    }

    fn handle_scale(self: Rc<Self>, mql: &MediaQueryList) {
        let weak_self = Rc::downgrade(&self);
        let scale = super::scale_factor(&self.window);

        // TODO: confirm/reproduce this problem, see:
        // <https://github.com/rust-windowing/winit/issues/2597>.
        // This should never happen, but if it does then apparently the scale factor didn't change.
        if mql.matches() {
            warn!(
                "media query tracking scale factor was triggered without a change:\nMedia Query: \
                 {}\nCurrent Scale: {scale}",
                mql.media(),
            );
            return;
        }

        let new_mql = Self::create_mql(&self.window, move |mql| {
            if let Some(rc_self) = weak_self.upgrade() {
                Self::handle_scale(rc_self, mql);
            }
        });
        self.mql.replace(new_mql);

        self.notify_scale.set(true);
        self.notify();
    }

    fn process_entry(&self, entries: Array) -> PhysicalSize<u32> {
        let entry: ResizeObserverEntry = entries.get(0).unchecked_into();

        // Safari doesn't support `devicePixelContentBoxSize`
        if !has_device_pixel_support() {
            let rect = entry.content_rect();

            return LogicalSize::new(rect.width(), rect.height())
                .to_physical(backend::scale_factor(&self.window));
        }

        let entry: ResizeObserverSize =
            entry.device_pixel_content_box_size().get(0).unchecked_into();

        let writing_mode = self.style.get("writing-mode");

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
            },
            // deprecated values
            "lr" | "lr-tb" | "rl" => true,
            "tb" | "tb-lr" | "tb-rl" => false,
            _ => {
                warn!("unrecognized `writing-mode`, assuming horizontal");
                true
            },
        };

        if horizontal {
            PhysicalSize::new(entry.inline_size() as u32, entry.block_size() as u32)
        } else {
            PhysicalSize::new(entry.block_size() as u32, entry.inline_size() as u32)
        }
    }
}

impl Drop for ResizeScaleInternal {
    fn drop(&mut self) {
        self.observer.disconnect();
    }
}

// TODO: Remove when Safari supports `devicePixelContentBoxSize`.
// See <https://bugs.webkit.org/show_bug.cgi?id=219005>.
pub fn has_device_pixel_support() -> bool {
    thread_local! {
        static DEVICE_PIXEL_SUPPORT: bool = {
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
        };
    }

    DEVICE_PIXEL_SUPPORT.with(|support| *support)
}
