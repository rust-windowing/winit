use super::bindings::{
    ResizeObserver, ResizeObserverBoxOptions, ResizeObserverEntry, ResizeObserverOptions,
    ResizeObserverSize,
};
use super::event;
use crate::dpi::{LogicalPosition, PhysicalPosition, PhysicalSize};
use crate::error::OsError as RootOE;
use crate::event::{ModifiersState, MouseButton, MouseScrollDelta, ScanCode, VirtualKeyCode};
use crate::platform_impl::{
    CanvasResizeChangedFlag, CanvasResizedArgs, OsError, PlatformSpecificWindowBuilderAttributes,
};

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{
    Event, FocusEvent, HtmlCanvasElement, KeyboardEvent, MediaQueryList, MediaQueryListEvent,
    MouseEvent, PointerEvent, WheelEvent,
};

pub struct Canvas {
    /// Note: resizing the HTMLCanvasElement should go through `backend::set_canvas_size` to ensure the DPI factor is maintained.
    raw: HtmlCanvasElement,
    pub(crate) auto_parent_size: bool,
    on_focus: Option<Closure<dyn FnMut(FocusEvent)>>,
    on_blur: Option<Closure<dyn FnMut(FocusEvent)>>,
    on_keyboard_release: Option<Closure<dyn FnMut(KeyboardEvent)>>,
    on_keyboard_press: Option<Closure<dyn FnMut(KeyboardEvent)>>,
    on_received_character: Option<Closure<dyn FnMut(KeyboardEvent)>>,
    on_cursor_leave: Option<Closure<dyn FnMut(PointerEvent)>>,
    on_cursor_enter: Option<Closure<dyn FnMut(PointerEvent)>>,
    on_cursor_move: Option<Closure<dyn FnMut(PointerEvent)>>,
    on_pointer_press: Option<Closure<dyn FnMut(PointerEvent)>>,
    on_pointer_release: Option<Closure<dyn FnMut(PointerEvent)>>,
    // Fallback events when pointer event support is missing
    on_mouse_leave: Option<Closure<dyn FnMut(MouseEvent)>>,
    on_mouse_enter: Option<Closure<dyn FnMut(MouseEvent)>>,
    on_mouse_move: Option<Closure<dyn FnMut(MouseEvent)>>,
    on_mouse_press: Option<Closure<dyn FnMut(MouseEvent)>>,
    on_mouse_release: Option<Closure<dyn FnMut(MouseEvent)>>,
    on_mouse_wheel: Option<Closure<dyn FnMut(WheelEvent)>>,
    on_fullscreen_change: Option<Closure<dyn FnMut(Event)>>,
    wants_fullscreen: Rc<RefCell<bool>>,
    on_dark_mode: Option<Closure<dyn FnMut(MediaQueryListEvent)>>,
    dpr_change_detector: Option<Rc<RefCell<DevicePixelRatioChangeDetector>>>,
    canvas_resize_observer: Option<Rc<RefCell<CanvasResizeObserver>>>,
}

impl Drop for Canvas {
    fn drop(&mut self) {
        self.raw.remove();
    }
}

impl Canvas {
    pub fn create(attr: PlatformSpecificWindowBuilderAttributes) -> Result<Self, RootOE> {
        let canvas = match attr.canvas {
            Some(canvas) => canvas,
            None => {
                let window = web_sys::window()
                    .ok_or(os_error!(OsError("Failed to obtain window".to_owned())))?;

                let document = window
                    .document()
                    .ok_or(os_error!(OsError("Failed to obtain document".to_owned())))?;

                document
                    .create_element("canvas")
                    .map_err(|_| os_error!(OsError("Failed to create canvas element".to_owned())))?
                    .unchecked_into()
            }
        };

        // A tabindex is needed in order to capture local keyboard events.
        // A "0" value means that the element should be focusable in
        // sequential keyboard navigation, but its order is defined by the
        // document's source order.
        // https://developer.mozilla.org/en-US/docs/Web/HTML/Global_attributes/tabindex
        canvas
            .set_attribute("tabindex", "0")
            .map_err(|_| os_error!(OsError("Failed to set a tabindex".to_owned())))?;

        Ok(Canvas {
            raw: canvas,
            auto_parent_size: attr.auto_parent_size,
            on_blur: None,
            on_focus: None,
            on_keyboard_release: None,
            on_keyboard_press: None,
            on_received_character: None,
            on_cursor_leave: None,
            on_cursor_enter: None,
            on_cursor_move: None,
            on_pointer_release: None,
            on_pointer_press: None,
            on_mouse_leave: None,
            on_mouse_enter: None,
            on_mouse_move: None,
            on_mouse_press: None,
            on_mouse_release: None,
            on_mouse_wheel: None,
            on_fullscreen_change: None,
            wants_fullscreen: Rc::new(RefCell::new(false)),
            on_dark_mode: None,
            dpr_change_detector: None,
            canvas_resize_observer: None,
        })
    }

    pub fn set_attribute(&self, attribute: &str, value: &str) {
        self.raw
            .set_attribute(attribute, value)
            .expect(&format!("Set attribute: {}", attribute));
    }

    pub fn position(&self) -> LogicalPosition<f64> {
        let bounds = self.raw.get_bounding_client_rect();

        LogicalPosition {
            x: bounds.x(),
            y: bounds.y(),
        }
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        PhysicalSize {
            width: self.raw.width(),
            height: self.raw.height(),
        }
    }

    pub fn raw(&self) -> &HtmlCanvasElement {
        &self.raw
    }

    pub fn on_blur<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(),
    {
        self.on_blur = Some(self.add_event("blur", move |_: FocusEvent| {
            handler();
        }));
    }

    pub fn on_focus<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(),
    {
        self.on_focus = Some(self.add_event("focus", move |_: FocusEvent| {
            handler();
        }));
    }

    pub fn on_keyboard_release<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(ScanCode, Option<VirtualKeyCode>, ModifiersState),
    {
        self.on_keyboard_release =
            Some(self.add_user_event("keyup", move |event: KeyboardEvent| {
                event.prevent_default();
                handler(
                    event::scan_code(&event),
                    event::virtual_key_code(&event),
                    event::keyboard_modifiers(&event),
                );
            }));
    }

    pub fn on_keyboard_press<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(ScanCode, Option<VirtualKeyCode>, ModifiersState),
    {
        self.on_keyboard_press =
            Some(self.add_user_event("keydown", move |event: KeyboardEvent| {
                event.prevent_default();
                handler(
                    event::scan_code(&event),
                    event::virtual_key_code(&event),
                    event::keyboard_modifiers(&event),
                );
            }));
    }

    pub fn on_received_character<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(char),
    {
        // TODO: Use `beforeinput`.
        //
        // The `keypress` event is deprecated, but there does not seem to be a
        // viable/compatible alternative as of now. `beforeinput` is still widely
        // unsupported.
        self.on_received_character = Some(self.add_user_event(
            "keypress",
            move |event: KeyboardEvent| {
                handler(event::codepoint(&event));
            },
        ));
    }

    pub fn on_cursor_leave<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32),
    {
        if has_pointer_event() {
            self.on_cursor_leave =
                Some(self.add_event("pointerout", move |event: PointerEvent| {
                    handler(event.pointer_id());
                }));
        } else {
            self.on_mouse_leave = Some(self.add_event("mouseout", move |_: MouseEvent| {
                handler(0);
            }));
        }
    }

    pub fn on_cursor_enter<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32),
    {
        if has_pointer_event() {
            self.on_cursor_enter =
                Some(self.add_event("pointerover", move |event: PointerEvent| {
                    handler(event.pointer_id());
                }));
        } else {
            self.on_mouse_enter = Some(self.add_event("mouseover", move |_: MouseEvent| {
                handler(0);
            }));
        }
    }

    pub fn on_mouse_release<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32, MouseButton, ModifiersState),
    {
        if has_pointer_event() {
            self.on_pointer_release = Some(self.add_user_event(
                "pointerup",
                move |event: PointerEvent| {
                    handler(
                        event.pointer_id(),
                        event::mouse_button(&event),
                        event::mouse_modifiers(&event),
                    );
                },
            ));
        } else {
            self.on_mouse_release =
                Some(self.add_user_event("mouseup", move |event: MouseEvent| {
                    handler(
                        0,
                        event::mouse_button(&event),
                        event::mouse_modifiers(&event),
                    );
                }));
        }
    }

    pub fn on_mouse_press<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32, MouseButton, ModifiersState),
    {
        if has_pointer_event() {
            self.on_pointer_press = Some(self.add_user_event(
                "pointerdown",
                move |event: PointerEvent| {
                    handler(
                        event.pointer_id(),
                        event::mouse_button(&event),
                        event::mouse_modifiers(&event),
                    );
                },
            ));
        } else {
            self.on_mouse_press =
                Some(self.add_user_event("mousedown", move |event: MouseEvent| {
                    handler(
                        0,
                        event::mouse_button(&event),
                        event::mouse_modifiers(&event),
                    );
                }));
        }
    }

    pub fn on_cursor_move<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32, PhysicalPosition<f64>, ModifiersState),
    {
        if has_pointer_event() {
            self.on_cursor_move =
                Some(self.add_event("pointermove", move |event: PointerEvent| {
                    handler(
                        event.pointer_id(),
                        event::mouse_position(&event).to_physical(super::scale_factor()),
                        event::mouse_modifiers(&event),
                    );
                }));
        } else {
            self.on_mouse_move = Some(self.add_event("mousemove", move |event: MouseEvent| {
                handler(
                    0,
                    event::mouse_position(&event).to_physical(super::scale_factor()),
                    event::mouse_modifiers(&event),
                );
            }));
        }
    }

    pub fn on_mouse_wheel<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32, MouseScrollDelta, ModifiersState),
    {
        self.on_mouse_wheel = Some(self.add_event("wheel", move |event: WheelEvent| {
            event.prevent_default();
            if let Some(delta) = event::mouse_scroll_delta(&event) {
                handler(0, delta, event::mouse_modifiers(&event));
            }
        }));
    }

    pub fn on_fullscreen_change<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(),
    {
        if !self.auto_parent_size {
            self.on_fullscreen_change =
                Some(self.add_event("fullscreenchange", move |_: Event| handler()));
        }
    }

    pub fn on_dark_mode<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(bool),
    {
        let window = web_sys::window().expect("Failed to obtain window");

        self.on_dark_mode = window
            .match_media("(prefers-color-scheme: dark)")
            .ok()
            .flatten()
            .and_then(|media| {
                let closure = Closure::wrap(Box::new(move |event: MediaQueryListEvent| {
                    handler(event.matches())
                }) as Box<dyn FnMut(_)>);

                media
                    .add_listener_with_opt_callback(Some(&closure.as_ref().unchecked_ref()))
                    .map(|_| closure)
                    .ok()
            });
    }

    pub fn on_device_pixel_ratio_change<F>(&mut self, handler: F)
    where
        F: 'static + FnMut(),
    {
        if !self.auto_parent_size {
            self.dpr_change_detector = Some(DevicePixelRatioChangeDetector::new(handler));
        }
    }

    pub(crate) fn on_size_or_scale_change<F>(&mut self, handler: F)
    where
        F: 'static + FnMut(CanvasResizedArgs),
    {
        if self.auto_parent_size {
            self.canvas_resize_observer =
                Some(CanvasResizeObserver::new(&self.raw, Box::new(handler)));
        }
    }

    fn add_event<E, F>(&self, event_name: &str, mut handler: F) -> Closure<dyn FnMut(E)>
    where
        E: 'static + AsRef<web_sys::Event> + wasm_bindgen::convert::FromWasmAbi,
        F: 'static + FnMut(E),
    {
        let closure = Closure::wrap(Box::new(move |event: E| {
            {
                let event_ref = event.as_ref();
                event_ref.stop_propagation();
                event_ref.cancel_bubble();
            }

            handler(event);
        }) as Box<dyn FnMut(E)>);

        self.raw
            .add_event_listener_with_callback(event_name, &closure.as_ref().unchecked_ref())
            .expect("Failed to add event listener with callback");

        closure
    }

    // The difference between add_event and add_user_event is that the latter has a special meaning
    // for browser security. A user event is a deliberate action by the user (like a mouse or key
    // press) and is the only time things like a fullscreen request may be successfully completed.)
    fn add_user_event<E, F>(&self, event_name: &str, mut handler: F) -> Closure<dyn FnMut(E)>
    where
        E: 'static + AsRef<web_sys::Event> + wasm_bindgen::convert::FromWasmAbi,
        F: 'static + FnMut(E),
    {
        let wants_fullscreen = self.wants_fullscreen.clone();
        let canvas = self.raw.clone();

        self.add_event(event_name, move |event: E| {
            handler(event);

            if *wants_fullscreen.borrow() {
                canvas
                    .request_fullscreen()
                    .expect("Failed to enter fullscreen");
                *wants_fullscreen.borrow_mut() = false;
            }
        })
    }

    pub fn request_fullscreen(&self) {
        *self.wants_fullscreen.borrow_mut() = true;
    }

    pub fn is_fullscreen(&self) -> bool {
        super::is_fullscreen(&self.raw)
    }
}

/// Returns whether pointer events are supported.
/// Used to decide whether to use pointer events
/// or plain mouse events. Note that Safari
/// doesn't support pointer events now.
fn has_pointer_event() -> bool {
    if let Some(window) = web_sys::window() {
        window.get("PointerEvent").is_some()
    } else {
        false
    }
}

/// This is a helper type to help manage the `MediaQueryList` used for detecting
/// changes of the `devicePixelRatio`.
struct DevicePixelRatioChangeDetector {
    callback: Box<dyn FnMut()>,
    closure: Option<Closure<dyn FnMut(MediaQueryListEvent)>>,
    mql: Option<MediaQueryList>,
}

impl DevicePixelRatioChangeDetector {
    fn new<F>(handler: F) -> Rc<RefCell<Self>>
    where
        F: 'static + FnMut(),
    {
        let new_self = Rc::new(RefCell::new(Self {
            callback: Box::new(handler),
            closure: None,
            mql: None,
        }));

        let cloned_self = new_self.clone();
        let closure = Closure::wrap(Box::new(move |event: MediaQueryListEvent| {
            cloned_self.borrow_mut().handler(event)
        }) as Box<dyn FnMut(_)>);

        let mql = Self::create_mql(&closure);
        {
            let mut borrowed_self = new_self.borrow_mut();
            borrowed_self.closure = Some(closure);
            borrowed_self.mql = mql;
        }
        new_self
    }

    fn create_mql(closure: &Closure<dyn FnMut(MediaQueryListEvent)>) -> Option<MediaQueryList> {
        let window = web_sys::window().expect("Failed to obtain window");
        let current_dpr = window.device_pixel_ratio();
        // This media query initially matches the current `devicePixelRatio`.
        // We add 0.0001 to the lower and upper bounds such that it won't fail
        // due to floating point precision limitations.
        let media_query = format!(
            "(min-resolution: {:.4}dppx) and (max-resolution: {:.4}dppx)",
            current_dpr - 0.0001,
            current_dpr + 0.0001,
        );
        web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(&format!(
            "DevicePixelRatioChangeDetector: new media query '{}'",
            media_query,
        )));
        window
            .match_media(&media_query)
            .ok()
            .flatten()
            .and_then(|mql| {
                assert_eq!(mql.matches(), true);
                mql.add_listener_with_opt_callback(Some(&closure.as_ref().unchecked_ref()))
                    .map(|_| mql)
                    .ok()
            })
    }

    fn handler(&mut self, event: MediaQueryListEvent) {
        assert_eq!(event.matches(), false);
        let closure = self
            .closure
            .as_ref()
            .expect("DevicePixelRatioChangeDetector::closure should not be None");
        let mql = self
            .mql
            .take()
            .expect("DevicePixelRatioChangeDetector::mql should not be None");
        mql.remove_listener_with_opt_callback(Some(closure.as_ref().unchecked_ref()))
            .expect("Failed to remove listener from MediaQueryList");
        (self.callback)();
        let new_mql = Self::create_mql(closure);
        self.mql = new_mql;
    }
}

impl Drop for DevicePixelRatioChangeDetector {
    fn drop(&mut self) {
        match (self.closure.as_ref(), self.mql.as_ref()) {
            (Some(closure), Some(mql)) => {
                let _ =
                    mql.remove_listener_with_opt_callback(Some(closure.as_ref().unchecked_ref()));
            }
            _ => {}
        }
    }
}

struct CanvasResizeObserver {
    callback: Box<dyn FnMut(CanvasResizedArgs)>,
    canvas: HtmlCanvasElement,
    is_device_pixel_content_box_supported: bool,
    /// The listener closure for the `ResizeObserver`. The callback argument is
    /// an array of `ResizeObserverList`.
    resize_observer_listener: Option<Closure<dyn FnMut(js_sys::Array)>>,
    /// The `ResizeObserver` for the canvas.
    resize_observer: Option<ResizeObserver>,
    /// The listener closure for the window resize event, in case `ResizeObserver`
    /// or 'device-pixel-content-box' is not supported.
    window_resize_listener: Option<Closure<dyn FnMut()>>,
    fullscreen_change_listener: Option<Closure<dyn FnMut()>>,
    last_width: u32,
    last_height: u32,
    last_dpr: f64,
    before_fullscreen_width: u32,
    before_fullscren_height: u32,
}

impl CanvasResizeObserver {
    fn new(
        canvas: &HtmlCanvasElement,
        callback: Box<dyn FnMut(CanvasResizedArgs)>,
    ) -> Rc<RefCell<Self>> {
        assert!(
            canvas
                .matches(":only-child")
                .expect("Fail to match selector `:only-child`"),
            "Only supports Canvas that is the only child of its parent",
        );
        super::set_canvas_style_property(&canvas, "width", "100%");
        super::set_canvas_style_property(&canvas, "height", "100%");
        super::set_canvas_style_property(&canvas, "display", "block");
        // super::set_canvas_style_property(&canvas, "position", "relative");
        super::set_canvas_style_property(&canvas, "position", "absolute");

        let window = web_sys::window().expect("Failed to obtain window");
        let dpr = window.device_pixel_ratio();

        // TODO: align the initial size maybe?
        // let width = (canvas.client_width() as f64 * dpr) as u32;
        // let height = (canvas.client_height() as f64 * dpr) as u32;

        let new_self = Rc::new(RefCell::new(Self {
            callback,
            canvas: canvas.clone(),
            is_device_pixel_content_box_supported: true,
            resize_observer_listener: None,
            resize_observer: None,
            window_resize_listener: None,
            fullscreen_change_listener: None,
            // last_width: width,
            last_width: 0,
            // last_height: height,
            last_height: 0,
            last_dpr: dpr,
            before_fullscreen_width: 0,
            before_fullscren_height: 0,
        }));

        let cloned_self = new_self.clone();
        // This is the handler for the `ResizeObserver`. The callback argument
        // is an array of `ResizeObserverList`.
        let closure = Closure::wrap(Box::new(move |entries: js_sys::Array| {
            cloned_self.borrow_mut().resize_observer_handler(entries)
        }) as Box<dyn FnMut(_)>);
        let resize_observer = ResizeObserver::new(&closure.as_ref().unchecked_ref());
        if let Ok(resize_observer) = resize_observer {
            let canvas_parent = canvas
                .parent_element()
                .expect("Failed to get parent element of Canvas");
            new_self.borrow_mut().resize_observer_listener = Some(closure);
            let observe_result = resize_observer.observe_with_options(
                &canvas_parent,
                ResizeObserverOptions::new().box_(ResizeObserverBoxOptions::DevicePixelContentBox),
            );
            if let Err(err) = observe_result {
                // 'device-pixel-content-box' not supported, falling back to 'border-box'.
                new_self.borrow_mut().is_device_pixel_content_box_supported = false;
                web_sys::console::warn_2(
                    &wasm_bindgen::JsValue::from_str(
                        "'device-pixel-content-box' not supported, falling back to 'border-box'.",
                    ),
                    &err,
                );
                resize_observer
                    .observe_with_options(
                        &canvas_parent,
                        ResizeObserverOptions::new().box_(ResizeObserverBoxOptions::BorderBox),
                    )
                    .expect("Failed to observe 'border-box'");
                // If we don't support `device-pixel-content-box`, then zooming
                // the page will not trigger the `ResizeObserver` callback if
                // the canvas CSS size has not changed even though the number
                // of native pixels it covers has.
                let cloned_self = new_self.clone();
                let closure =
                    Closure::wrap(
                        Box::new(move || cloned_self.borrow_mut().window_resize_handler())
                            as Box<dyn FnMut()>,
                    );
                window
                    .add_event_listener_with_callback("resize", &closure.as_ref().unchecked_ref())
                    .expect("Failed to add `resize` listener");
                new_self.borrow_mut().window_resize_listener = Some(closure);
            }
            new_self.borrow_mut().resize_observer = Some(resize_observer);
        } else {
            web_sys::console::warn_1(&wasm_bindgen::JsValue::from_str(
                "ResizeObserver not supported, falling back to only using resize event.",
            ));
            new_self.borrow_mut().is_device_pixel_content_box_supported = false;

            let cloned_self = new_self.clone();
            let closure =
                Closure::wrap(
                    Box::new(move || cloned_self.borrow_mut().window_resize_handler())
                        as Box<dyn FnMut()>,
                );
            window
                .add_event_listener_with_callback("resize", &closure.as_ref().unchecked_ref())
                .expect("Failed to add `resize` listener");
            new_self.borrow_mut().window_resize_listener = Some(closure);

            // TODO: Should we also use setTimeout to check for size changes?

            // Run the handler once to set the initial size.
            new_self.borrow_mut().window_resize_handler()
        }

        let cloned_self = new_self.clone();
        let closure =
            Closure::wrap(
                Box::new(move || cloned_self.borrow_mut().fullscreen_change_handler())
                    as Box<dyn FnMut()>,
            );
        canvas
            .add_event_listener_with_callback("fullscreenchange", &closure.as_ref().unchecked_ref())
            .expect("Failed to add `fullscreenchange` listener");
        new_self.borrow_mut().fullscreen_change_listener = Some(closure);
        new_self
    }

    fn resize_observer_handler(&mut self, entries: js_sys::Array) {
        for entry in entries.iter() {
            let entry = entry
                .dyn_into::<ResizeObserverEntry>()
                .expect("Failed to get `ResizeObserverEntry`");
            let (width, height) = if self.is_device_pixel_content_box_supported {
                let size = entry
                    .device_pixel_content_box_size()
                    .get(0)
                    .dyn_into::<ResizeObserverSize>()
                    .expect("Failed to get ResizeObserverSize");
                (size.inline_size() as u32, size.block_size() as u32)
            } else {
                snap_to_parent_pixels(&self.canvas)
            };
            web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(&format!(
                "resized {}x{}",
                width, height
            )));
            self.maybe_handle_resize(width, height, super::scale_factor());
        }
    }

    fn window_resize_handler(&mut self) {
        assert_eq!(self.is_device_pixel_content_box_supported, false);
        let (width, height) = snap_to_parent_pixels(&self.canvas);
        web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(&format!(
            "resized (window) {}x{}",
            width, height
        )));
        self.maybe_handle_resize(width, height, super::scale_factor());
    }

    fn fullscreen_change_handler(&mut self) {
        web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(&format!(
            "Fullscreen change"
        )));
        let dpr = super::scale_factor();
        // If the canvas is marked as fullscreen, it is moving *into* fullscreen
        // If it is not, it is moving *out of* fullscreen
        let (width, height) = if super::is_fullscreen(&self.canvas) {
            let size = super::window_size().to_physical(dpr);
            (size.width, size.height)
        } else if self.is_device_pixel_content_box_supported {
            (self.before_fullscreen_width, self.before_fullscren_height)
        } else {
            snap_to_parent_pixels(&self.canvas)
        };
        self.maybe_handle_resize(width, height, dpr);
    }

    fn maybe_handle_resize(&mut self, width: u32, height: u32, dpr: f64) {
        let new_size = {
            // If the canvas is marked as fullscreen, it is moving *into* fullscreen
            // If it is not, it is moving *out of* fullscreen
            if super::is_fullscreen(&self.canvas) {
                web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(&format!(
                    "In fullscreen"
                )));
                // If we're in fullscreen, we don't care about the size of the
                // parent element.
                super::window_size().to_physical(dpr)
            } else {
                self.before_fullscreen_width = width;
                self.before_fullscren_height = height;
                PhysicalSize::new(width, height)
            }
        };
        let width = new_size.width;
        let height = new_size.height;
        let maybe_changed_flag = {
            // We do expect the devicePixelRatio to be exactly equal unless changed.
            #[allow(clippy::float_cmp)]
            if self.last_dpr != dpr {
                Some(CanvasResizeChangedFlag::SizeAndDevicePixelRatioChanged)
            } else if self.last_width != new_size.width || self.last_height != new_size.height {
                Some(CanvasResizeChangedFlag::SizeChanged)
            } else {
                None
            }
        };
        if let Some(changed_flag) = maybe_changed_flag {
            self.canvas.set_width(width);
            self.canvas.set_height(height);
            self.last_width = width;
            self.last_height = height;
            self.last_dpr = dpr;
            (self.callback)(CanvasResizedArgs {
                size: new_size,
                device_pixel_ratio: dpr,
                changed_flag,
            })
        }
    }
}

impl Drop for CanvasResizeObserver {
    fn drop(&mut self) {
        if let Some(resize_observer) = self.resize_observer.take() {
            resize_observer.disconnect();
        }
        if let Some(listener) = self.window_resize_listener.take() {
            web_sys::window().map(|window| {
                let _ = window.remove_event_listener_with_callback(
                    "resize",
                    listener.as_ref().unchecked_ref(),
                );
            });
        }
        if let Some(listener) = self.fullscreen_change_listener.take() {
            let _ = self
                .canvas
                .remove_event_listener_with_callback("resize", listener.as_ref().unchecked_ref());
        }
    }
}

/// This function attempts to snap the canvas element to the pixel grid based
/// on its parent element. This function should *never* be used if the browser
/// supports `device-pixel-content-box` in `ResizeObserver`.
fn snap_to_parent_pixels(canvas: &HtmlCanvasElement) -> (u32, u32) {
    fn client_to_device_dim(dpr: f64, dim: f64) -> f64 {
        (dim * dpr).round()
    }

    let canvas_parent = &canvas
        .parent_element()
        .expect("Failed to get parent element of Canvas");
    let client_rect = canvas_parent.get_bounding_client_rect();
    let dpr = super::scale_factor();

    web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(&format!(
        "bounding client rect top/left: {} {}",
        client_rect.top() * dpr,
        client_rect.left() * dpr
    )));
    let width = client_to_device_dim(dpr, client_rect.width());
    let height = client_to_device_dim(dpr, client_rect.height());

    super::set_canvas_style_property(canvas, "width", &format!("{}px", width / dpr));
    super::set_canvas_style_property(canvas, "height", &format!("{}px", height / dpr));

    (width as u32, height as u32)
}
