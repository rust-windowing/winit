use super::event;
use crate::dpi::{LogicalPosition, PhysicalPosition, PhysicalSize};
use crate::error::OsError as RootOE;
use crate::event::{ModifiersState, MouseButton, MouseScrollDelta, ScanCode, VirtualKeyCode};
use crate::platform_impl::{OsError, PlatformSpecificWindowBuilderAttributes};

use std::cell::RefCell;
use std::rc::Rc;
use stdweb::js;
use stdweb::traits::IPointerEvent;
use stdweb::unstable::TryInto;
use stdweb::web::event::{
    BlurEvent, ConcreteEvent, FocusEvent, FullscreenChangeEvent, IEvent, KeyDownEvent,
    KeyPressEvent, KeyUpEvent, MouseWheelEvent, PointerDownEvent, PointerMoveEvent,
    PointerOutEvent, PointerOverEvent, PointerUpEvent,
};
use stdweb::web::html_element::CanvasElement;
use stdweb::web::{
    document, EventListenerHandle, IChildNode, IElement, IEventTarget, IHtmlElement,
};

pub struct Canvas {
    /// Note: resizing the CanvasElement should go through `backend::set_canvas_size` to ensure the DPI factor is maintained.
    raw: CanvasElement,
    on_focus: Option<EventListenerHandle>,
    on_blur: Option<EventListenerHandle>,
    on_keyboard_release: Option<EventListenerHandle>,
    on_keyboard_press: Option<EventListenerHandle>,
    on_received_character: Option<EventListenerHandle>,
    on_cursor_leave: Option<EventListenerHandle>,
    on_cursor_enter: Option<EventListenerHandle>,
    on_cursor_move: Option<EventListenerHandle>,
    on_mouse_press: Option<EventListenerHandle>,
    on_mouse_release: Option<EventListenerHandle>,
    on_mouse_wheel: Option<EventListenerHandle>,
    on_fullscreen_change: Option<EventListenerHandle>,
    wants_fullscreen: Rc<RefCell<bool>>,
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
            None => document()
                .create_element("canvas")
                .map_err(|_| os_error!(OsError("Failed to create canvas element".to_owned())))?
                .try_into()
                .map_err(|_| os_error!(OsError("Failed to create canvas element".to_owned())))?,
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
            on_blur: None,
            on_focus: None,
            on_keyboard_release: None,
            on_keyboard_press: None,
            on_received_character: None,
            on_cursor_leave: None,
            on_cursor_enter: None,
            on_cursor_move: None,
            on_mouse_release: None,
            on_mouse_press: None,
            on_mouse_wheel: None,
            on_fullscreen_change: None,
            wants_fullscreen: Rc::new(RefCell::new(false)),
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
            x: bounds.get_x(),
            y: bounds.get_y(),
        }
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        PhysicalSize {
            width: self.raw.width() as u32,
            height: self.raw.height() as u32,
        }
    }

    pub fn raw(&self) -> &CanvasElement {
        &self.raw
    }

    pub fn on_blur<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(),
    {
        self.on_blur = Some(self.add_event(move |_: BlurEvent| {
            handler();
        }));
    }

    pub fn on_focus<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(),
    {
        self.on_focus = Some(self.add_event(move |_: FocusEvent| {
            handler();
        }));
    }

    pub fn on_keyboard_release<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(ScanCode, Option<VirtualKeyCode>, ModifiersState),
    {
        self.on_keyboard_release = Some(self.add_user_event(move |event: KeyUpEvent| {
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
        self.on_keyboard_press = Some(self.add_user_event(move |event: KeyDownEvent| {
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
        self.on_received_character = Some(self.add_user_event(move |event: KeyPressEvent| {
            handler(event::codepoint(&event));
        }));
    }

    pub fn on_cursor_leave<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32),
    {
        self.on_cursor_leave = Some(self.add_event(move |event: PointerOutEvent| {
            handler(event.pointer_id());
        }));
    }

    pub fn on_cursor_enter<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32),
    {
        self.on_cursor_enter = Some(self.add_event(move |event: PointerOverEvent| {
            handler(event.pointer_id());
        }));
    }

    pub fn on_mouse_release<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32, MouseButton, ModifiersState),
    {
        self.on_mouse_release = Some(self.add_user_event(move |event: PointerUpEvent| {
            handler(
                event.pointer_id(),
                event::mouse_button(&event),
                event::mouse_modifiers(&event),
            );
        }));
    }

    pub fn on_mouse_press<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32, PhysicalPosition<f64>, MouseButton, ModifiersState),
    {
        let canvas = self.raw.clone();
        self.on_mouse_press = Some(self.add_user_event(move |event: PointerDownEvent| {
            handler(
                event.pointer_id(),
                event::mouse_position(&event).to_physical(super::scale_factor()),
                event::mouse_button(&event),
                event::mouse_modifiers(&event),
            );
            canvas
                .set_pointer_capture(event.pointer_id())
                .expect("Failed to set pointer capture");
        }));
    }

    pub fn on_cursor_move<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32, PhysicalPosition<f64>, ModifiersState),
    {
        // todo
        self.on_cursor_move = Some(self.add_event(move |event: PointerMoveEvent| {
            handler(
                event.pointer_id(),
                event::mouse_position(&event).to_physical(super::scale_factor()),
                event::mouse_modifiers(&event),
            );
        }));
    }

    pub fn on_mouse_wheel<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32, MouseScrollDelta, ModifiersState),
    {
        self.on_mouse_wheel = Some(self.add_event(move |event: MouseWheelEvent| {
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
        self.on_fullscreen_change = Some(self.add_event(move |_: FullscreenChangeEvent| handler()));
    }

    pub fn on_dark_mode<F>(&mut self, handler: F)
    where
        F: 'static + FnMut(bool),
    {
        // TODO: upstream to stdweb
        js! {
            var handler = @{handler};

            if (window.matchMedia) {
                window.matchMedia("(prefers-color-scheme: dark)").addListener(function(e) {
                    handler(event.matches)
                });
            }
        }
    }

    fn add_event<E, F>(&self, mut handler: F) -> EventListenerHandle
    where
        E: ConcreteEvent,
        F: 'static + FnMut(E),
    {
        self.raw.add_event_listener(move |event: E| {
            event.stop_propagation();
            event.cancel_bubble();

            handler(event);
        })
    }

    // The difference between add_event and add_user_event is that the latter has a special meaning
    // for browser security. A user event is a deliberate action by the user (like a mouse or key
    // press) and is the only time things like a fullscreen request may be successfully completed.)
    fn add_user_event<E, F>(&self, mut handler: F) -> EventListenerHandle
    where
        E: ConcreteEvent,
        F: 'static + FnMut(E),
    {
        let wants_fullscreen = self.wants_fullscreen.clone();
        let canvas = self.raw.clone();

        self.add_event(move |event: E| {
            handler(event);

            if *wants_fullscreen.borrow() {
                canvas.request_fullscreen();
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
