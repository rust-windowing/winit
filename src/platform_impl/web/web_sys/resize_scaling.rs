use js_sys::{Array, Object};
use once_cell::unsync::Lazy;
use wasm_bindgen::prelude::{wasm_bindgen, Closure};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
    HtmlCanvasElement, MediaQueryList, ResizeObserver, ResizeObserverBoxOptions,
    ResizeObserverEntry, ResizeObserverOptions, ResizeObserverSize, Window,
};

use crate::dpi::{LogicalSize, PhysicalSize};

use super::super::backend;
use super::media_query_handle::MediaQueryListHandle;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct ResizeScaleHandle(Rc<RefCell<ResizeScaleInternal>>);

impl ResizeScaleHandle {
    pub(crate) fn new<S, R>(
        window: Window,
        canvas: HtmlCanvasElement,
        scale_handler: S,
        resize_handler: R,
    ) -> Self
    where
        S: 'static + FnMut(PhysicalSize<u32>, f64),
        R: 'static + FnMut(PhysicalSize<u32>),
    {
        Self(ResizeScaleInternal::new(
            window,
            canvas,
            scale_handler,
            resize_handler,
        ))
    }

    pub(crate) fn notify_resize(&self) {
        self.0.borrow_mut().notify()
    }
}

/// This is a helper type to help manage the `MediaQueryList` used for detecting
/// changes of the `devicePixelRatio`.
struct ResizeScaleInternal {
    window: Window,
    canvas: HtmlCanvasElement,
    mql: MediaQueryListHandle,
    observer: ResizeObserver,
    _observer_closure: Closure<dyn FnMut(Array, ResizeObserver)>,
    scale_handler: Box<dyn FnMut(PhysicalSize<u32>, f64)>,
    resize_handler: Box<dyn FnMut(PhysicalSize<u32>)>,
    notify_scale: Cell<bool>,
}

impl ResizeScaleInternal {
    fn new<S, R>(
        window: Window,
        canvas: HtmlCanvasElement,
        scale_handler: S,
        resize_handler: R,
    ) -> Rc<RefCell<Self>>
    where
        S: 'static + FnMut(PhysicalSize<u32>, f64),
        R: 'static + FnMut(PhysicalSize<u32>),
    {
        Rc::<RefCell<ResizeScaleInternal>>::new_cyclic(|weak_self| {
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
                if let Some(rc_self) = weak_self.upgrade() {
                    let mut this = rc_self.borrow_mut();

                    let size = Self::process_entry(&this.window, &this.canvas, entries);

                    if this.notify_scale.replace(false) {
                        let scale = backend::scale_factor(&this.window);
                        (this.scale_handler)(size, scale)
                    } else {
                        (this.resize_handler)(size)
                    }
                }
            });
            let observer = Self::create_observer(&canvas, observer_closure.as_ref());

            RefCell::new(Self {
                window,
                canvas,
                mql,
                observer,
                _observer_closure: observer_closure,
                scale_handler: Box::new(scale_handler),
                resize_handler: Box::new(resize_handler),
                notify_scale: Cell::new(false),
            })
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
        assert!(
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
            observer.observe_with_options(
                canvas,
                ResizeObserverOptions::new().box_(ResizeObserverBoxOptions::DevicePixelContentBox),
            );
        } else {
            observer.observe(canvas);
        }

        observer
    }

    fn notify(&mut self) {
        let style = self
            .window
            .get_computed_style(&self.canvas)
            .expect("Failed to obtain computed style")
            // this can't fail: we aren't using a pseudo-element
            .expect("Invalid pseudo-element");

        let document = self.window.document().expect("Failed to obtain document");

        if !document.contains(Some(&self.canvas))
            || style.get_property_value("display").unwrap() == "none"
        {
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
            backend::style_size_property(&style, "width"),
            backend::style_size_property(&style, "height"),
        );

        if style.get_property_value("box-sizing").unwrap() == "border-box" {
            size.width -= backend::style_size_property(&style, "border-left-width")
                + backend::style_size_property(&style, "border-right-width")
                + backend::style_size_property(&style, "padding-left")
                + backend::style_size_property(&style, "padding-right");
            size.height -= backend::style_size_property(&style, "border-top-width")
                + backend::style_size_property(&style, "border-bottom-width")
                + backend::style_size_property(&style, "padding-top")
                + backend::style_size_property(&style, "padding-bottom");
        }

        let size = size.to_physical(backend::scale_factor(&self.window));

        if self.notify_scale.replace(false) {
            let scale = backend::scale_factor(&self.window);
            (self.scale_handler)(size, scale)
        } else {
            (self.resize_handler)(size)
        }
    }

    fn handle_scale(this: Rc<RefCell<Self>>, mql: &MediaQueryList) {
        let weak_self = Rc::downgrade(&this);
        let mut this = this.borrow_mut();
        let scale = super::scale_factor(&this.window);

        // TODO: confirm/reproduce this problem, see:
        // <https://github.com/rust-windowing/winit/issues/2597>.
        // This should never happen, but if it does then apparently the scale factor didn't change.
        if mql.matches() {
            warn!(
                "media query tracking scale factor was triggered without a change:\n\
                Media Query: {}\n\
                Current Scale: {scale}",
                mql.media(),
            );
            return;
        }

        let new_mql = Self::create_mql(&this.window, move |mql| {
            if let Some(rc_self) = weak_self.upgrade() {
                Self::handle_scale(rc_self, mql);
            }
        });
        this.mql = new_mql;

        this.notify_scale.set(true);
        this.notify();
    }

    fn process_entry(
        window: &Window,
        canvas: &HtmlCanvasElement,
        entries: Array,
    ) -> PhysicalSize<u32> {
        let entry: ResizeObserverEntry = entries.get(0).unchecked_into();

        // Safari doesn't support `devicePixelContentBoxSize`
        if !has_device_pixel_support() {
            let rect = entry.content_rect();

            return LogicalSize::new(rect.width(), rect.height())
                .to_physical(backend::scale_factor(window));
        }

        let entry: ResizeObserverSize = entry
            .device_pixel_content_box_size()
            .get(0)
            .unchecked_into();

        let style = window
            .get_computed_style(canvas)
            .expect("Failed to get computed style of canvas")
            // this can only be empty if we provided an invalid `pseudoElt`
            .expect("`getComputedStyle` can not be empty");

        let writing_mode = style
            .get_property_value("writing-mode")
            .expect("`writing-mode` is a valid CSS property");

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

impl Drop for ResizeScaleInternal {
    fn drop(&mut self) {
        self.observer.disconnect();
    }
}

// TODO: Remove when Safari supports `devicePixelContentBoxSize`.
// See <https://bugs.webkit.org/show_bug.cgi?id=219005>.
pub fn has_device_pixel_support() -> bool {
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
