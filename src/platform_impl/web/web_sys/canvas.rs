use std::cell::{Cell, RefCell};
use std::ops::Deref;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use smol_str::SmolStr;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{
    CssStyleDeclaration, Document, Event, FocusEvent, HtmlCanvasElement, KeyboardEvent, Navigator,
    PointerEvent, WheelEvent,
};

use super::super::cursor::CursorHandler;
use super::super::main_thread::MainThreadMarker;
use super::animation_frame::AnimationFrameHandler;
use super::event_handle::EventListenerHandle;
use super::intersection_handle::IntersectionObserverHandle;
use super::media_query_handle::MediaQueryListHandle;
use super::pointer::PointerHandler;
use super::{event, fullscreen, ResizeScaleHandle};
use crate::dpi::{LogicalPosition, PhysicalPosition, PhysicalSize};
use crate::error::RequestError;
use crate::event::{
    ButtonSource, DeviceId, ElementState, MouseScrollDelta, PointerKind, PointerSource,
    SurfaceSizeWriter,
};
use crate::keyboard::{Key, KeyLocation, ModifiersState, PhysicalKey};
use crate::platform_impl::Fullscreen;
use crate::window::{WindowAttributes, WindowId};

#[allow(dead_code)]
pub struct Canvas {
    main_thread: MainThreadMarker,
    common: Common,
    id: WindowId,
    pub has_focus: Rc<Cell<bool>>,
    pub prevent_default: Rc<Cell<bool>>,
    pub is_intersecting: Cell<Option<bool>>,
    pub cursor: CursorHandler,
    handlers: RefCell<Handlers>,
}

struct Handlers {
    animation_frame_handler: AnimationFrameHandler,
    on_touch_start: Option<EventListenerHandle<dyn FnMut(Event)>>,
    on_focus: Option<EventListenerHandle<dyn FnMut(FocusEvent)>>,
    on_blur: Option<EventListenerHandle<dyn FnMut(FocusEvent)>>,
    on_keyboard_release: Option<EventListenerHandle<dyn FnMut(KeyboardEvent)>>,
    on_keyboard_press: Option<EventListenerHandle<dyn FnMut(KeyboardEvent)>>,
    on_mouse_wheel: Option<EventListenerHandle<dyn FnMut(WheelEvent)>>,
    on_dark_mode: Option<MediaQueryListHandle>,
    pointer_handler: PointerHandler,
    on_resize_scale: Option<ResizeScaleHandle>,
    on_intersect: Option<IntersectionObserverHandle>,
    on_touch_end: Option<EventListenerHandle<dyn FnMut(Event)>>,
    on_context_menu: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
}

pub struct Common {
    pub window: web_sys::Window,
    navigator: Navigator,
    pub document: Document,
    /// Note: resizing the HTMLCanvasElement should go through `backend::set_canvas_size` to ensure
    /// the DPI factor is maintained. Note: this is read-only because we use a pointer to this
    /// for [`WindowHandle`][rwh_06::WindowHandle].
    raw: Rc<HtmlCanvasElement>,
    style: Style,
    old_size: Rc<Cell<PhysicalSize<u32>>>,
    current_size: Rc<Cell<PhysicalSize<u32>>>,
}

#[derive(Clone, Debug)]
pub struct Style {
    read: CssStyleDeclaration,
    write: CssStyleDeclaration,
}

impl Canvas {
    pub(crate) fn create(
        main_thread: MainThreadMarker,
        id: WindowId,
        window: web_sys::Window,
        navigator: Navigator,
        document: Document,
        attr: WindowAttributes,
    ) -> Result<Self, RequestError> {
        let canvas = match attr.platform_specific.canvas.map(Arc::try_unwrap) {
            Some(Ok(canvas)) => canvas.into_inner(main_thread),
            Some(Err(canvas)) => canvas.get(main_thread).clone(),
            None => document
                .create_element("canvas")
                .map_err(|_| os_error!("Failed to create canvas element"))?
                .unchecked_into(),
        };

        if attr.platform_specific.append && !document.contains(Some(&canvas)) {
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
        if attr.platform_specific.focusable {
            canvas
                .set_attribute("tabindex", "0")
                .map_err(|_| os_error!("Failed to set a tabindex"))?;
        }

        let style = Style::new(&window, &canvas);

        let cursor = CursorHandler::new(main_thread, canvas.clone(), style.clone());

        let common = Common {
            window: window.clone(),
            document: document.clone(),
            navigator,
            raw: Rc::new(canvas.clone()),
            style,
            old_size: Rc::default(),
            current_size: Rc::default(),
        };

        if let Some(size) = attr.surface_size {
            let size = size.to_logical(super::scale_factor(&common.window));
            super::set_canvas_size(&common.document, &common.raw, &common.style, size);
        }

        if let Some(size) = attr.min_surface_size {
            let size = size.to_logical(super::scale_factor(&common.window));
            super::set_canvas_min_size(&common.document, &common.raw, &common.style, Some(size));
        }

        if let Some(size) = attr.max_surface_size {
            let size = size.to_logical(super::scale_factor(&common.window));
            super::set_canvas_max_size(&common.document, &common.raw, &common.style, Some(size));
        }

        if let Some(position) = attr.position {
            let position = position.to_logical(super::scale_factor(&common.window));
            super::set_canvas_position(&common.document, &common.raw, &common.style, position);
        }

        if let Some(fullscreen) = attr.fullscreen {
            fullscreen::request_fullscreen(
                main_thread,
                &window,
                &document,
                &canvas,
                fullscreen.into(),
            );
        }

        if attr.active {
            let _ = common.raw.focus();
        }

        Ok(Canvas {
            main_thread,
            common,
            id,
            has_focus: Rc::new(Cell::new(false)),
            prevent_default: Rc::new(Cell::new(attr.platform_specific.prevent_default)),
            is_intersecting: Cell::new(None),
            cursor,
            handlers: RefCell::new(Handlers {
                animation_frame_handler: AnimationFrameHandler::new(window),
                on_touch_start: None,
                on_blur: None,
                on_focus: None,
                on_keyboard_release: None,
                on_keyboard_press: None,
                on_mouse_wheel: None,
                on_dark_mode: None,
                pointer_handler: PointerHandler::new(),
                on_resize_scale: None,
                on_intersect: None,
                on_touch_end: None,
                on_context_menu: None,
            }),
        })
    }

    pub fn set_attribute(&self, attribute: &str, value: &str) {
        self.common
            .raw
            .set_attribute(attribute, value)
            .unwrap_or_else(|err| panic!("error: {err:?}\nSet attribute: {attribute}"))
    }

    pub fn position(&self) -> LogicalPosition<f64> {
        let bounds = self.common.raw.get_bounding_client_rect();
        let mut position = LogicalPosition { x: bounds.x(), y: bounds.y() };

        if self.document().contains(Some(self.raw())) && self.style().get("display") != "none" {
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
    pub fn surface_size(&self) -> PhysicalSize<u32> {
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
    pub fn navigator(&self) -> &Navigator {
        &self.common.navigator
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
    pub fn style(&self) -> &Style {
        &self.common.style
    }

    pub fn on_touch_start(&self) {
        let prevent_default = Rc::clone(&self.prevent_default);
        self.handlers.borrow_mut().on_touch_start =
            Some(self.common.add_event("touchstart", move |event: Event| {
                if prevent_default.get() {
                    event.prevent_default();
                }
            }));
    }

    pub fn on_blur<F>(&self, mut handler: F)
    where
        F: 'static + FnMut(),
    {
        self.handlers.borrow_mut().on_blur =
            Some(self.common.add_event("blur", move |_: FocusEvent| {
                handler();
            }));
    }

    pub fn on_focus<F>(&self, mut handler: F)
    where
        F: 'static + FnMut(),
    {
        self.handlers.borrow_mut().on_focus =
            Some(self.common.add_event("focus", move |_: FocusEvent| {
                handler();
            }));
    }

    pub fn on_keyboard_release<F>(&self, mut handler: F)
    where
        F: 'static + FnMut(PhysicalKey, Key, Option<SmolStr>, KeyLocation, bool, ModifiersState),
    {
        let prevent_default = Rc::clone(&self.prevent_default);
        self.handlers.borrow_mut().on_keyboard_release =
            Some(self.common.add_event("keyup", move |event: KeyboardEvent| {
                if prevent_default.get() {
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
            }));
    }

    pub fn on_keyboard_press<F>(&self, mut handler: F)
    where
        F: 'static + FnMut(PhysicalKey, Key, Option<SmolStr>, KeyLocation, bool, ModifiersState),
    {
        let prevent_default = Rc::clone(&self.prevent_default);
        self.handlers.borrow_mut().on_keyboard_press =
            Some(self.common.add_event("keydown", move |event: KeyboardEvent| {
                if prevent_default.get() {
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
            }));
    }

    pub fn on_pointer_leave<F>(&self, handler: F)
    where
        F: 'static
            + FnMut(ModifiersState, Option<DeviceId>, bool, PhysicalPosition<f64>, PointerKind),
    {
        self.handlers.borrow_mut().pointer_handler.on_pointer_leave(&self.common, handler)
    }

    pub fn on_pointer_enter<F>(&self, handler: F)
    where
        F: 'static
            + FnMut(ModifiersState, Option<DeviceId>, bool, PhysicalPosition<f64>, PointerKind),
    {
        self.handlers.borrow_mut().pointer_handler.on_pointer_enter(&self.common, handler)
    }

    pub fn on_pointer_release<C>(&self, handler: C)
    where
        C: 'static
            + FnMut(ModifiersState, Option<DeviceId>, bool, PhysicalPosition<f64>, ButtonSource),
    {
        self.handlers.borrow_mut().pointer_handler.on_pointer_release(&self.common, handler)
    }

    pub fn on_pointer_press<C>(&self, handler: C)
    where
        C: 'static
            + FnMut(ModifiersState, Option<DeviceId>, bool, PhysicalPosition<f64>, ButtonSource),
    {
        self.handlers.borrow_mut().pointer_handler.on_pointer_press(
            &self.common,
            handler,
            Rc::clone(&self.prevent_default),
        )
    }

    pub fn on_pointer_move<C, B>(&self, cursor_handler: C, button_handler: B)
    where
        C: 'static
            + FnMut(
                Option<DeviceId>,
                &mut dyn Iterator<
                    Item = (ModifiersState, bool, PhysicalPosition<f64>, PointerSource),
                >,
            ),
        B: 'static
            + FnMut(
                ModifiersState,
                Option<DeviceId>,
                bool,
                PhysicalPosition<f64>,
                ElementState,
                ButtonSource,
            ),
    {
        self.handlers.borrow_mut().pointer_handler.on_pointer_move(
            &self.common,
            cursor_handler,
            button_handler,
            Rc::clone(&self.prevent_default),
        )
    }

    pub fn on_mouse_wheel<F>(&self, mut handler: F)
    where
        F: 'static + FnMut(MouseScrollDelta, ModifiersState),
    {
        let window = self.common.window.clone();
        let prevent_default = Rc::clone(&self.prevent_default);
        self.handlers.borrow_mut().on_mouse_wheel =
            Some(self.common.add_event("wheel", move |event: WheelEvent| {
                if prevent_default.get() {
                    event.prevent_default();
                }

                if let Some(delta) = event::mouse_scroll_delta(&window, &event) {
                    let modifiers = event::mouse_modifiers(&event);
                    handler(delta, modifiers);
                }
            }));
    }

    pub fn on_dark_mode<F>(&self, mut handler: F)
    where
        F: 'static + FnMut(bool),
    {
        self.handlers.borrow_mut().on_dark_mode = Some(MediaQueryListHandle::new(
            &self.common.window,
            "(prefers-color-scheme: dark)",
            move |mql| handler(mql.matches()),
        ));
    }

    pub(crate) fn on_resize_scale<S, R>(&self, scale_handler: S, size_handler: R)
    where
        S: 'static + Fn(PhysicalSize<u32>, f64),
        R: 'static + Fn(PhysicalSize<u32>),
    {
        self.handlers.borrow_mut().on_resize_scale = Some(ResizeScaleHandle::new(
            self.window().clone(),
            self.document().clone(),
            self.raw().clone(),
            self.style().clone(),
            scale_handler,
            size_handler,
        ));
    }

    pub(crate) fn on_intersection<F>(&self, handler: F)
    where
        F: 'static + FnMut(bool),
    {
        self.handlers.borrow_mut().on_intersect =
            Some(IntersectionObserverHandle::new(self.raw(), handler));
    }

    pub(crate) fn on_animation_frame<F>(&self, f: F)
    where
        F: 'static + FnMut(),
    {
        self.handlers.borrow_mut().animation_frame_handler.on_animation_frame(f)
    }

    pub(crate) fn on_context_menu(&self) {
        let prevent_default = Rc::clone(&self.prevent_default);
        self.handlers.borrow_mut().on_context_menu =
            Some(self.common.add_event("contextmenu", move |event: PointerEvent| {
                if prevent_default.get() {
                    event.prevent_default();
                }
            }));
    }

    pub(crate) fn request_fullscreen(&self, fullscreen: Fullscreen) {
        fullscreen::request_fullscreen(
            self.main_thread,
            self.window(),
            self.document(),
            self.raw(),
            fullscreen,
        );
    }

    pub fn exit_fullscreen(&self) {
        fullscreen::exit_fullscreen(self.document(), self.raw());
    }

    pub fn is_fullscreen(&self) -> bool {
        fullscreen::is_fullscreen(self.document(), self.raw())
    }

    pub fn request_animation_frame(&self) {
        self.handlers.borrow().animation_frame_handler.request();
    }

    pub(crate) fn handle_scale_change(
        &self,
        runner: &super::super::event_loop::runner::Shared,
        event_handler: impl FnOnce(crate::event::Event),
        current_size: PhysicalSize<u32>,
        scale: f64,
    ) {
        // First, we send the `ScaleFactorChanged` event:
        self.set_current_size(current_size);
        let new_size = {
            let new_size = Arc::new(Mutex::new(current_size));
            event_handler(crate::event::Event::WindowEvent {
                window_id: self.id,
                event: crate::event::WindowEvent::ScaleFactorChanged {
                    scale_factor: scale,
                    surface_size_writer: SurfaceSizeWriter::new(Arc::downgrade(&new_size)),
                },
            });

            let new_size = *new_size.lock().unwrap();
            new_size
        };

        if current_size != new_size {
            // Then we resize the canvas to the new size, a new `SurfaceResized` event will be sent
            // by the `ResizeObserver`:
            let new_size = new_size.to_logical(scale);
            super::set_canvas_size(self.document(), self.raw(), self.style(), new_size);

            // Set the size might not trigger the event because the calculation is inaccurate.
            self.handlers
                .borrow()
                .on_resize_scale
                .as_ref()
                .expect("expected Window to still be active")
                .notify_resize();
        } else if self.old_size() != new_size {
            // Then we at least send a resized event.
            self.set_old_size(new_size);
            runner.send_event(crate::event::Event::WindowEvent {
                window_id: self.id,
                event: crate::event::WindowEvent::SurfaceResized(new_size),
            })
        }
    }

    pub fn remove_listeners(&self) {
        let mut handlers = self.handlers.borrow_mut();
        handlers.on_touch_start.take();
        handlers.on_focus.take();
        handlers.on_blur.take();
        handlers.on_keyboard_release.take();
        handlers.on_keyboard_press.take();
        handlers.on_mouse_wheel.take();
        handlers.on_dark_mode.take();
        handlers.pointer_handler.remove_listeners();
        handlers.on_resize_scale = None;
        handlers.on_intersect = None;
        handlers.animation_frame_handler.cancel();
        handlers.on_touch_end = None;
        handlers.on_context_menu = None;
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
        EventListenerHandle::new(self.raw.deref().clone(), event_name, Closure::new(handler))
    }

    pub fn raw(&self) -> &HtmlCanvasElement {
        &self.raw
    }
}

impl Style {
    fn new(window: &web_sys::Window, canvas: &HtmlCanvasElement) -> Self {
        #[allow(clippy::disallowed_methods)]
        let read = window
            .get_computed_style(canvas)
            .expect("Failed to obtain computed style")
            // this can't fail: we aren't using a pseudo-element
            .expect("Invalid pseudo-element");

        #[allow(clippy::disallowed_methods)]
        let write = canvas.style();

        Self { read, write }
    }

    pub(crate) fn get(&self, property: &str) -> String {
        self.read.get_property_value(property).expect("Invalid property")
    }

    pub(crate) fn remove(&self, property: &str) {
        self.write.remove_property(property).expect("Property is read only");
    }

    pub(crate) fn set(&self, property: &str, value: &str) {
        self.write.set_property(property, value).expect("Property is read only");
    }
}
