use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use js_sys::Promise;
use smol_str::SmolStr;
use wasm_bindgen::{closure::Closure, JsCast};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    CssStyleDeclaration, Document, Event, FocusEvent, HtmlCanvasElement, KeyboardEvent, WheelEvent,
};

use crate::dpi::{LogicalPosition, PhysicalPosition, PhysicalSize};
use crate::error::OsError as RootOE;
use crate::event::{Force, InnerSizeWriter, MouseButton, MouseScrollDelta};
use crate::keyboard::{Key, KeyCode, KeyLocation, ModifiersState};
use crate::platform_impl::{OsError, PlatformSpecificWindowBuilderAttributes};
use crate::window::{WindowAttributes, WindowId as RootWindowId};

use super::super::WindowId;
use super::animation_frame::AnimationFrameHandler;
use super::event_handle::EventListenerHandle;
use super::intersection_handle::IntersectionObserverHandle;
use super::media_query_handle::MediaQueryListHandle;
use super::pointer::PointerHandler;
use super::{event, fullscreen, ButtonsState, ResizeScaleHandle};

#[allow(dead_code)]
pub struct Canvas {
    common: Common,
    id: WindowId,
    pub has_focus: Arc<AtomicBool>,
    pub is_intersecting: Option<bool>,
    on_touch_start: Option<EventListenerHandle<dyn FnMut(Event)>>,
    on_touch_end: Option<EventListenerHandle<dyn FnMut(Event)>>,
    on_focus: Option<EventListenerHandle<dyn FnMut(FocusEvent)>>,
    on_blur: Option<EventListenerHandle<dyn FnMut(FocusEvent)>>,
    on_keyboard_release: Option<EventListenerHandle<dyn FnMut(KeyboardEvent)>>,
    on_keyboard_press: Option<EventListenerHandle<dyn FnMut(KeyboardEvent)>>,
    on_mouse_wheel: Option<EventListenerHandle<dyn FnMut(WheelEvent)>>,
    on_dark_mode: Option<MediaQueryListHandle>,
    pointer_handler: PointerHandler,
    on_resize_scale: Option<ResizeScaleHandle>,
    on_intersect: Option<IntersectionObserverHandle>,
    animation_frame_handler: AnimationFrameHandler,
}

pub struct Common {
    pub window: web_sys::Window,
    pub document: Document,
    /// Note: resizing the HTMLCanvasElement should go through `backend::set_canvas_size` to ensure the DPI factor is maintained.
    pub raw: HtmlCanvasElement,
    style: CssStyleDeclaration,
    old_size: Rc<Cell<PhysicalSize<u32>>>,
    current_size: Rc<Cell<PhysicalSize<u32>>>,
    wants_fullscreen: Rc<RefCell<bool>>,
}

impl Canvas {
    pub fn create(
        id: WindowId,
        window: web_sys::Window,
        document: Document,
        attr: &WindowAttributes,
        platform_attr: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Self, RootOE> {
        let canvas = match platform_attr.canvas {
            Some(canvas) => canvas,
            None => document
                .create_element("canvas")
                .map_err(|_| os_error!(OsError("Failed to create canvas element".to_owned())))?
                .unchecked_into(),
        };

        if platform_attr.append && !document.contains(Some(&canvas)) {
            document
                .body()
                .expect("Failed to get body from document")
                .append_child(&canvas)
                .expect("Failed to append canvas to body");
        }

        // A tabindex is needed in order to capture local keyboard events.
        // A "0" value means that the element should be focusable in
        // sequential keyboard navigation, but its order is defined by the
        // document's source order.
        // https://developer.mozilla.org/en-US/docs/Web/HTML/Global_attributes/tabindex
        if platform_attr.focusable {
            canvas
                .set_attribute("tabindex", "0")
                .map_err(|_| os_error!(OsError("Failed to set a tabindex".to_owned())))?;
        }

        #[allow(clippy::disallowed_methods)]
        let style = window
            .get_computed_style(&canvas)
            .expect("Failed to obtain computed style")
            // this can't fail: we aren't using a pseudo-element
            .expect("Invalid pseudo-element");

        let common = Common {
            window: window.clone(),
            document,
            raw: canvas,
            style,
            old_size: Rc::default(),
            current_size: Rc::default(),
            wants_fullscreen: Rc::new(RefCell::new(false)),
        };

        if let Some(size) = attr.inner_size {
            let size = size.to_logical(super::scale_factor(&common.window));
            super::set_canvas_size(&common.document, &common.raw, &common.style, size);
        }

        if let Some(size) = attr.min_inner_size {
            let size = size.to_logical(super::scale_factor(&common.window));
            super::set_canvas_min_size(&common.document, &common.raw, &common.style, Some(size));
        }

        if let Some(size) = attr.max_inner_size {
            let size = size.to_logical(super::scale_factor(&common.window));
            super::set_canvas_max_size(&common.document, &common.raw, &common.style, Some(size));
        }

        if let Some(position) = attr.position {
            let position = position.to_logical(super::scale_factor(&common.window));
            super::set_canvas_position(&common.document, &common.raw, &common.style, position);
        }

        if attr.fullscreen.is_some() {
            common.request_fullscreen();
        }

        if attr.active {
            let _ = common.raw.focus();
        }

        Ok(Canvas {
            common,
            id,
            has_focus: Arc::new(AtomicBool::new(false)),
            is_intersecting: None,
            on_touch_start: None,
            on_touch_end: None,
            on_blur: None,
            on_focus: None,
            on_keyboard_release: None,
            on_keyboard_press: None,
            on_mouse_wheel: None,
            on_dark_mode: None,
            pointer_handler: PointerHandler::new(),
            on_resize_scale: None,
            on_intersect: None,
            animation_frame_handler: AnimationFrameHandler::new(window),
        })
    }

    pub fn set_cursor_lock(&self, lock: bool) -> Result<(), RootOE> {
        if lock {
            self.raw().request_pointer_lock();
        } else {
            self.common.document.exit_pointer_lock();
        }
        Ok(())
    }

    pub fn set_attribute(&self, attribute: &str, value: &str) {
        self.common
            .raw
            .set_attribute(attribute, value)
            .unwrap_or_else(|err| panic!("error: {err:?}\nSet attribute: {attribute}"))
    }

    pub fn position(&self) -> LogicalPosition<f64> {
        let bounds = self.common.raw.get_bounding_client_rect();
        let mut position = LogicalPosition {
            x: bounds.x(),
            y: bounds.y(),
        };

        if self.document().contains(Some(self.raw()))
            && self.style().get_property_value("display").unwrap() != "none"
        {
            position.x += super::style_size_property(self.style(), "border-left-width")
                + super::style_size_property(self.style(), "padding-left");
            position.y += super::style_size_property(self.style(), "border-top-width")
                + super::style_size_property(self.style(), "padding-top");
        }

        position
    }

    #[inline]
    pub fn old_size(&self) -> PhysicalSize<u32> {
        self.common.old_size.get()
    }

    #[inline]
    pub fn inner_size(&self) -> PhysicalSize<u32> {
        self.common.current_size.get()
    }

    #[inline]
    pub fn set_old_size(&self, size: PhysicalSize<u32>) {
        self.common.old_size.set(size)
    }

    #[inline]
    pub fn set_current_size(&self, size: PhysicalSize<u32>) {
        self.common.current_size.set(size)
    }

    #[inline]
    pub fn window(&self) -> &web_sys::Window {
        &self.common.window
    }

    #[inline]
    pub fn document(&self) -> &Document {
        &self.common.document
    }

    #[inline]
    pub fn raw(&self) -> &HtmlCanvasElement {
        &self.common.raw
    }

    #[inline]
    pub fn style(&self) -> &CssStyleDeclaration {
        &self.common.style
    }

    pub fn on_touch_start(&mut self, prevent_default: bool) {
        self.on_touch_start = Some(self.common.add_event("touchstart", move |event: Event| {
            if prevent_default {
                event.prevent_default();
            }
        }));
    }

    pub fn on_blur<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(),
    {
        self.on_blur = Some(self.common.add_event("blur", move |_: FocusEvent| {
            handler();
        }));
    }

    pub fn on_focus<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(),
    {
        self.on_focus = Some(self.common.add_event("focus", move |_: FocusEvent| {
            handler();
        }));
    }

    pub fn on_keyboard_release<F>(&mut self, mut handler: F, prevent_default: bool)
    where
        F: 'static + FnMut(KeyCode, Key, Option<SmolStr>, KeyLocation, bool, ModifiersState),
    {
        self.on_keyboard_release = Some(self.common.add_user_event(
            "keyup",
            move |event: KeyboardEvent| {
                if prevent_default {
                    event.prevent_default();
                }
                let key = event::key(&event);
                let modifiers = event::keyboard_modifiers(&event);
                handler(
                    event::key_code(&event),
                    key,
                    event::key_text(&event),
                    event::key_location(&event),
                    event.repeat(),
                    modifiers,
                );
            },
        ));
    }

    pub fn on_keyboard_press<F>(&mut self, mut handler: F, prevent_default: bool)
    where
        F: 'static + FnMut(KeyCode, Key, Option<SmolStr>, KeyLocation, bool, ModifiersState),
    {
        self.on_keyboard_press = Some(self.common.add_user_event(
            "keydown",
            move |event: KeyboardEvent| {
                if prevent_default {
                    event.prevent_default();
                }
                let key = event::key(&event);
                let modifiers = event::keyboard_modifiers(&event);
                handler(
                    event::key_code(&event),
                    key,
                    event::key_text(&event),
                    event::key_location(&event),
                    event.repeat(),
                    modifiers,
                );
            },
        ));
    }

    pub fn on_cursor_leave<F>(&mut self, handler: F)
    where
        F: 'static + FnMut(ModifiersState, Option<i32>),
    {
        self.pointer_handler.on_cursor_leave(&self.common, handler)
    }

    pub fn on_cursor_enter<F>(&mut self, handler: F)
    where
        F: 'static + FnMut(ModifiersState, Option<i32>),
    {
        self.pointer_handler.on_cursor_enter(&self.common, handler)
    }

    pub fn on_mouse_release<MOD, M, T>(
        &mut self,
        modifier_handler: MOD,
        mouse_handler: M,
        touch_handler: T,
    ) where
        MOD: 'static + FnMut(ModifiersState),
        M: 'static + FnMut(ModifiersState, i32, PhysicalPosition<f64>, MouseButton),
        T: 'static + FnMut(ModifiersState, i32, PhysicalPosition<f64>, Force),
    {
        self.pointer_handler.on_mouse_release(
            &self.common,
            modifier_handler,
            mouse_handler,
            touch_handler,
        )
    }

    pub fn on_mouse_press<MOD, M, T>(
        &mut self,
        modifier_handler: MOD,
        mouse_handler: M,
        touch_handler: T,
        prevent_default: bool,
    ) where
        MOD: 'static + FnMut(ModifiersState),
        M: 'static + FnMut(ModifiersState, i32, PhysicalPosition<f64>, MouseButton),
        T: 'static + FnMut(ModifiersState, i32, PhysicalPosition<f64>, Force),
    {
        self.pointer_handler.on_mouse_press(
            &self.common,
            modifier_handler,
            mouse_handler,
            touch_handler,
            prevent_default,
        )
    }

    pub fn on_cursor_move<MOD, M, T, B>(
        &mut self,
        modifier_handler: MOD,
        mouse_handler: M,
        touch_handler: T,
        button_handler: B,
        prevent_default: bool,
    ) where
        MOD: 'static + FnMut(ModifiersState),
        M: 'static + FnMut(ModifiersState, i32, &mut dyn Iterator<Item = PhysicalPosition<f64>>),
        T: 'static
            + FnMut(ModifiersState, i32, &mut dyn Iterator<Item = (PhysicalPosition<f64>, Force)>),
        B: 'static + FnMut(ModifiersState, i32, PhysicalPosition<f64>, ButtonsState, MouseButton),
    {
        self.pointer_handler.on_cursor_move(
            &self.common,
            modifier_handler,
            mouse_handler,
            touch_handler,
            button_handler,
            prevent_default,
        )
    }

    pub fn on_touch_cancel<F>(&mut self, handler: F)
    where
        F: 'static + FnMut(i32, PhysicalPosition<f64>, Force),
    {
        self.pointer_handler.on_touch_cancel(&self.common, handler)
    }

    pub fn on_mouse_wheel<F>(&mut self, mut handler: F, prevent_default: bool)
    where
        F: 'static + FnMut(i32, MouseScrollDelta, ModifiersState),
    {
        let window = self.common.window.clone();
        self.on_mouse_wheel = Some(self.common.add_event("wheel", move |event: WheelEvent| {
            if prevent_default {
                event.prevent_default();
            }

            if let Some(delta) = event::mouse_scroll_delta(&window, &event) {
                let modifiers = event::mouse_modifiers(&event);
                handler(0, delta, modifiers);
            }
        }));
    }

    pub fn on_dark_mode<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(bool),
    {
        self.on_dark_mode = Some(MediaQueryListHandle::new(
            &self.common.window,
            "(prefers-color-scheme: dark)",
            move |mql| handler(mql.matches()),
        ));
    }

    pub(crate) fn on_resize_scale<S, R>(&mut self, scale_handler: S, size_handler: R)
    where
        S: 'static + FnMut(PhysicalSize<u32>, f64),
        R: 'static + FnMut(PhysicalSize<u32>),
    {
        self.on_resize_scale = Some(ResizeScaleHandle::new(
            self.window().clone(),
            self.document().clone(),
            self.raw().clone(),
            self.style().clone(),
            scale_handler,
            size_handler,
        ));
    }

    pub(crate) fn on_intersection<F>(&mut self, handler: F)
    where
        F: 'static + FnMut(bool),
    {
        self.on_intersect = Some(IntersectionObserverHandle::new(self.raw(), handler));
    }

    pub(crate) fn on_animation_frame<F>(&mut self, f: F)
    where
        F: 'static + FnMut(),
    {
        self.animation_frame_handler.on_animation_frame(f)
    }

    pub fn request_fullscreen(&self) {
        self.common.request_fullscreen()
    }

    pub fn is_fullscreen(&self) -> bool {
        self.common.is_fullscreen()
    }

    pub fn request_animation_frame(&self) {
        self.animation_frame_handler.request();
    }

    pub(crate) fn handle_scale_change<T: 'static>(
        &self,
        runner: &super::super::event_loop::runner::Shared<T>,
        event_handler: impl FnOnce(crate::event::Event<T>),
        current_size: PhysicalSize<u32>,
        scale: f64,
    ) {
        // First, we send the `ScaleFactorChanged` event:
        self.set_current_size(current_size);
        let new_size = {
            let new_size = Arc::new(Mutex::new(current_size));
            event_handler(crate::event::Event::WindowEvent {
                window_id: RootWindowId(self.id),
                event: crate::event::WindowEvent::ScaleFactorChanged {
                    scale_factor: scale,
                    inner_size_writer: InnerSizeWriter::new(Arc::downgrade(&new_size)),
                },
            });

            let new_size = *new_size.lock().unwrap();
            new_size
        };

        if current_size != new_size {
            // Then we resize the canvas to the new size, a new
            // `Resized` event will be sent by the `ResizeObserver`:
            let new_size = new_size.to_logical(scale);
            super::set_canvas_size(self.document(), self.raw(), self.style(), new_size);

            // Set the size might not trigger the event because the calculation is inaccurate.
            self.on_resize_scale
                .as_ref()
                .expect("expected Window to still be active")
                .notify_resize();
        } else if self.old_size() != new_size {
            // Then we at least send a resized event.
            self.set_old_size(new_size);
            runner.send_event(crate::event::Event::WindowEvent {
                window_id: RootWindowId(self.id),
                event: crate::event::WindowEvent::Resized(new_size),
            })
        }
    }

    pub fn remove_listeners(&mut self) {
        self.on_focus = None;
        self.on_blur = None;
        self.on_keyboard_release = None;
        self.on_keyboard_press = None;
        self.on_mouse_wheel = None;
        self.on_dark_mode = None;
        self.pointer_handler.remove_listeners();
        self.on_resize_scale = None;
        self.on_intersect = None;
    }
}

impl Common {
    pub fn add_event<E, F>(
        &self,
        event_name: &'static str,
        handler: F,
    ) -> EventListenerHandle<dyn FnMut(E)>
    where
        E: 'static + AsRef<web_sys::Event> + wasm_bindgen::convert::FromWasmAbi,
        F: 'static + FnMut(E),
    {
        EventListenerHandle::new(self.raw.clone(), event_name, Closure::new(handler))
    }

    // The difference between add_event and add_user_event is that the latter has a special meaning
    // for browser security. A user event is a deliberate action by the user (like a mouse or key
    // press) and is the only time things like a fullscreen request may be successfully completed.)
    pub fn add_user_event<E, F>(
        &self,
        event_name: &'static str,
        mut handler: F,
    ) -> EventListenerHandle<dyn FnMut(E)>
    where
        E: 'static + AsRef<web_sys::Event> + wasm_bindgen::convert::FromWasmAbi,
        F: 'static + FnMut(E),
    {
        let wants_fullscreen = self.wants_fullscreen.clone();
        let canvas = self.raw.clone();

        self.add_event(event_name, move |event: E| {
            handler(event);

            if *wants_fullscreen.borrow() {
                fullscreen::request_fullscreen(&canvas).expect("Failed to enter fullscreen");
                *wants_fullscreen.borrow_mut() = false;
            }
        })
    }

    pub fn request_fullscreen(&self) {
        // This should return a `Promise`, but Safari v<16.4 is not up-to-date with the spec.
        match fullscreen::request_fullscreen(&self.raw) {
            Ok(value) if !value.is_undefined() => {
                let promise: Promise = value.unchecked_into();
                let wants_fullscreen = self.wants_fullscreen.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if JsFuture::from(promise).await.is_err() {
                        *wants_fullscreen.borrow_mut() = true
                    }
                });
            }
            // We are on Safari v<16.4, let's try again on the next transient activation.
            _ => *self.wants_fullscreen.borrow_mut() = true,
        }
    }

    pub fn is_fullscreen(&self) -> bool {
        super::is_fullscreen(&self.document, &self.raw)
    }
}
