use {CreationError, WindowEvent as Event, MouseCursor, CursorState};
use CreationError::OsError;
use libc;

use WindowAttributes;
use native_monitor::NativeMonitorId;
use os::macos::ActivationPolicy;

use objc;
use objc::runtime::{Class, Object, Sel, BOOL, YES, NO};
use objc::declare::ClassDecl;

use cocoa;
use cocoa::base::{id, nil};
use cocoa::foundation::{NSPoint, NSRect, NSSize, NSString, NSUInteger};
use cocoa::appkit::{self, NSApplication, NSColor, NSView, NSWindow};

use core_graphics::display::{CGAssociateMouseAndMouseCursorPosition, CGMainDisplayID, CGDisplayPixelsHigh, CGWarpMouseCursorPosition};

use std;
use std::ops::Deref;
use std::os::raw::c_void;

use os::macos::WindowExt;


#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(pub usize);

struct DelegateState {
    view: IdRef,
    window: IdRef,
    events_loop: std::sync::Weak<super::EventsLoop>,
}

pub struct WindowDelegate {
    state: Box<DelegateState>,
    _this: IdRef,
}

impl WindowDelegate {
    /// Get the delegate class, initiailizing it neccessary
    fn class() -> *const Class {
        use std::os::raw::c_void;

        // Emits an event via the `EventsLoop`.
        //
        // If the `EventsLoop` callback is `Some` the event is immediately emitted via the callback.
        //
        // If it is `None`, it is pushed to the back of the `EventsLoop`'s `pending_events` deque.
        fn emit_event(state: &mut DelegateState, window_event: Event) {
            let window_id = get_window_id(*state.window);
            let event = ::Event::WindowEvent {
                window_id: ::WindowId(window_id),
                event: window_event,
            };

            if let Some(events_loop) = state.events_loop.upgrade() {
                if let Ok(mut callback) = events_loop.callback.lock() {
                    if let Some(callback) = callback.as_mut() {
                        callback(event);
                        return;
                    }
                }

                if let Ok(mut pending_events) = events_loop.pending_events.lock() {
                    pending_events.push_back(event);
                }
            }
        }

        extern fn window_should_close(this: &Object, _: Sel, _: id) -> BOOL {
            unsafe {
                let state: *mut c_void = *this.get_ivar("glutinState");
                let state = &mut *(state as *mut DelegateState);
                emit_event(state, Event::Closed);
            }
            YES
        }

        extern fn window_did_resize(this: &Object, _: Sel, _: id) {
            unsafe {
                let state: *mut c_void = *this.get_ivar("glutinState");
                let state = &mut *(state as *mut DelegateState);
                let rect = NSView::frame(*state.view);
                let scale_factor = NSWindow::backingScaleFactor(*state.window) as f32;
                let width = (scale_factor * rect.size.width as f32) as u32;
                let height = (scale_factor * rect.size.height as f32) as u32;
                emit_event(state, Event::Resized(width, height));
            }
        }

        extern fn window_did_become_key(this: &Object, _: Sel, _: id) {
            unsafe {
                // TODO: center the cursor if the window had mouse grab when it
                // lost focus
                let state: *mut c_void = *this.get_ivar("glutinState");
                let state = &mut *(state as *mut DelegateState);
                emit_event(state, Event::Focused(true));
            }
        }

        extern fn window_did_resign_key(this: &Object, _: Sel, _: id) {
            unsafe {
                let state: *mut c_void = *this.get_ivar("glutinState");
                let state = &mut *(state as *mut DelegateState);
                emit_event(state, Event::Focused(false));
            }
        }

        static mut DELEGATE_CLASS: *const Class = 0 as *const Class;
        static INIT: std::sync::Once = std::sync::ONCE_INIT;

        INIT.call_once(|| unsafe {
            // Create new NSWindowDelegate
            let superclass = Class::get("NSObject").unwrap();
            let mut decl = ClassDecl::new("GlutinWindowDelegate", superclass).unwrap();

            // Add callback methods
            decl.add_method(sel!(windowShouldClose:),
                window_should_close as extern fn(&Object, Sel, id) -> BOOL);
            decl.add_method(sel!(windowDidResize:),
                window_did_resize as extern fn(&Object, Sel, id));

            decl.add_method(sel!(windowDidBecomeKey:),
                window_did_become_key as extern fn(&Object, Sel, id));
            decl.add_method(sel!(windowDidResignKey:),
                window_did_resign_key as extern fn(&Object, Sel, id));

            // Store internal state as user data
            decl.add_ivar::<*mut c_void>("glutinState");

            DELEGATE_CLASS = decl.register();
        });

        unsafe {
            DELEGATE_CLASS
        }
    }

    fn new(state: DelegateState) -> WindowDelegate {
        // Box the state so we can give a pointer to it
        let mut state = Box::new(state);
        let state_ptr: *mut DelegateState = &mut *state;
        unsafe {
            let delegate = IdRef::new(msg_send![WindowDelegate::class(), new]);

            (&mut **delegate).set_ivar("glutinState", state_ptr as *mut ::std::os::raw::c_void);
            let _: () = msg_send![*state.window, setDelegate:*delegate];

            WindowDelegate { state: state, _this: delegate }
        }
    }
}

impl Drop for WindowDelegate {
    fn drop(&mut self) {
        unsafe {
            // Nil the window's delegate so it doesn't still reference us
            let _: () = msg_send![*self.state.window, setDelegate:nil];
        }
    }
}

#[derive(Clone, Default)]
pub struct PlatformSpecificWindowBuilderAttributes {
    pub activation_policy: ActivationPolicy,
}

pub struct Window {
    pub view: IdRef,
    pub window: IdRef,
    pub delegate: WindowDelegate,
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

impl Drop for Window {
    fn drop(&mut self) {
        // Remove this window from the `EventLoop`s list of windows.
        let id = self.id();
        if let Some(ev) = self.delegate.state.events_loop.upgrade() {
            let mut windows = ev.windows.lock().unwrap();
            windows.retain(|w| w.id() != id)
        }
    }
}

impl WindowExt for Window {
    #[inline]
    fn get_nswindow(&self) -> *mut c_void {
        *self.window as *mut c_void
    }

    #[inline]
    fn get_nsview(&self) -> *mut c_void {
        *self.view as *mut c_void
    }
}

impl Window {
    pub fn new(events_loop: std::sync::Weak<super::EventsLoop>,
               win_attribs: &WindowAttributes,
               pl_attribs: &PlatformSpecificWindowBuilderAttributes)
               -> Result<Window, CreationError>
    {
        let app = match Window::create_app(pl_attribs.activation_policy) {
            Some(app) => app,
            None      => { return Err(OsError(format!("Couldn't create NSApplication"))); },
        };

        let window = match Window::create_window(win_attribs)
        {
            Some(window) => window,
            None         => { return Err(OsError(format!("Couldn't create NSWindow"))); },
        };
        let view = match Window::create_view(*window) {
            Some(view) => view,
            None       => { return Err(OsError(format!("Couldn't create NSView"))); },
        };

        unsafe {
            if win_attribs.transparent {
                (*window as id).setOpaque_(NO);
                (*window as id).setBackgroundColor_(NSColor::clearColor(nil));
            }

            app.activateIgnoringOtherApps_(YES);
            if win_attribs.visible {
                window.makeKeyAndOrderFront_(nil);
            } else {
                window.makeKeyWindow();
            }

            if let Some((width, height)) = win_attribs.min_dimensions {
                nswindow_set_min_dimensions(window.0, width.into(), height.into());
            }

            if let Some((width, height)) = win_attribs.max_dimensions {
                nswindow_set_max_dimensions(window.0, width.into(), height.into());
            }
        }

        let ds = DelegateState {
            view: view.clone(),
            window: window.clone(),
            events_loop: events_loop,
        };

        let window = Window {
            view: view,
            window: window,
            delegate: WindowDelegate::new(ds),
        };

        Ok(window)
    }

    pub fn id(&self) -> Id {
        get_window_id(*self.window)
    }

    fn create_app(activation_policy: ActivationPolicy) -> Option<id> {
        unsafe {
            let app = appkit::NSApp();
            if app == nil {
                None
            } else {
                app.setActivationPolicy_(activation_policy.into());
                app.finishLaunching();
                Some(app)
            }
        }
    }

    fn create_window(attrs: &WindowAttributes) -> Option<IdRef> {
        unsafe {
            let screen = match attrs.monitor {
                Some(ref monitor_id) => {
                    let native_id = match monitor_id.get_native_identifier() {
                        NativeMonitorId::Numeric(num) => num,
                        _ => panic!("OS X monitors should always have a numeric native ID")
                    };
                    let matching_screen = {
                        let screens = appkit::NSScreen::screens(nil);
                        let count: NSUInteger = msg_send![screens, count];
                        let key = IdRef::new(NSString::alloc(nil).init_str("NSScreenNumber"));
                        let mut matching_screen: Option<id> = None;
                        for i in 0..count {
                            let screen = msg_send![screens, objectAtIndex:i as NSUInteger];
                            let device_description = appkit::NSScreen::deviceDescription(screen);
                            let value: id = msg_send![device_description, objectForKey:*key];
                            if value != nil {
                                let screen_number: NSUInteger = msg_send![value, unsignedIntegerValue];
                                if screen_number as u32 == native_id {
                                    matching_screen = Some(screen);
                                    break;
                                }
                            }
                        }
                        matching_screen
                    };
                    Some(matching_screen.unwrap_or(appkit::NSScreen::mainScreen(nil)))
                },
                None => None
            };
            let frame = match screen {
                Some(screen) => appkit::NSScreen::frame(screen),
                None => {
                    let (width, height) = attrs.dimensions.unwrap_or((800, 600));
                    NSRect::new(NSPoint::new(0., 0.), NSSize::new(width as f64, height as f64))
                }
            };

            let masks = if screen.is_some() || attrs.transparent {
                // Fullscreen or transparent window
                appkit::NSBorderlessWindowMask as NSUInteger |
                appkit::NSResizableWindowMask as NSUInteger |
                appkit::NSTitledWindowMask as NSUInteger
            } else if attrs.decorations {
                // Classic opaque window with titlebar
                appkit::NSClosableWindowMask as NSUInteger |
                appkit::NSMiniaturizableWindowMask as NSUInteger |
                appkit::NSResizableWindowMask as NSUInteger |
                appkit::NSTitledWindowMask as NSUInteger
            } else {
                // Opaque window without a titlebar
                appkit::NSClosableWindowMask as NSUInteger |
                appkit::NSMiniaturizableWindowMask as NSUInteger |
                appkit::NSResizableWindowMask as NSUInteger |
                appkit::NSTitledWindowMask as NSUInteger |
                appkit::NSFullSizeContentViewWindowMask as NSUInteger
            };

            let window = IdRef::new(NSWindow::alloc(nil).initWithContentRect_styleMask_backing_defer_(
                frame,
                masks,
                appkit::NSBackingStoreBuffered,
                NO,
            ));
            window.non_nil().map(|window| {
                let title = IdRef::new(NSString::alloc(nil).init_str(&attrs.title));
                window.setReleasedWhenClosed_(NO);
                window.setTitle_(*title);
                window.setAcceptsMouseMovedEvents_(YES);

                if !attrs.decorations {
                    window.setTitleVisibility_(appkit::NSWindowTitleVisibility::NSWindowTitleHidden);
                    window.setTitlebarAppearsTransparent_(YES);
                }

                if screen.is_some() {
                    window.setLevel_(appkit::NSMainMenuWindowLevel as i64 + 1);
                }
                else {
                    window.center();
                }
                window
            })
        }
    }

    fn create_view(window: id) -> Option<IdRef> {
        unsafe {
            let view = IdRef::new(NSView::alloc(nil).init());
            view.non_nil().map(|view| {
                view.setWantsBestResolutionOpenGLSurface_(YES);
                window.setContentView_(*view);
                view
            })
        }
    }

    pub fn set_title(&self, title: &str) {
        unsafe {
            let title = IdRef::new(NSString::alloc(nil).init_str(title));
            self.window.setTitle_(*title);
        }
    }

    #[inline]
    pub fn show(&self) {
        unsafe { NSWindow::makeKeyAndOrderFront_(*self.window, nil); }
    }

    #[inline]
    pub fn hide(&self) {
        unsafe { NSWindow::orderOut_(*self.window, nil); }
    }

    pub fn get_position(&self) -> Option<(i32, i32)> {
        unsafe {
            let content_rect = NSWindow::contentRectForFrameRect_(*self.window, NSWindow::frame(*self.window));

            // TODO: consider extrapolating the calculations for the y axis to
            // a private method
            Some((content_rect.origin.x as i32, (CGDisplayPixelsHigh(CGMainDisplayID()) as f64 - (content_rect.origin.y + content_rect.size.height)) as i32))
        }
    }

    pub fn set_position(&self, x: i32, y: i32) {
        unsafe {
            let frame = NSWindow::frame(*self.view);

            // NOTE: `setFrameOrigin` might not give desirable results when
            // setting window, as it treats bottom left as origin.
            // `setFrameTopLeftPoint` treats top left as origin (duh), but
            // does not equal the value returned by `get_window_position`
            // (there is a difference by 22 for me on yosemite)

            // TODO: consider extrapolating the calculations for the y axis to
            // a private method
            let dummy = NSRect::new(NSPoint::new(x as f64, CGDisplayPixelsHigh(CGMainDisplayID()) as f64 - (frame.size.height + y as f64)), NSSize::new(0f64, 0f64));
            let conv = NSWindow::frameRectForContentRect_(*self.window, dummy);

            // NSWindow::setFrameTopLeftPoint_(*self.window, conv.origin);
            NSWindow::setFrameOrigin_(*self.window, conv.origin);
        }
    }

    #[inline]
    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        unsafe {
            let view_frame = NSView::frame(*self.view);
            Some((view_frame.size.width as u32, view_frame.size.height as u32))
        }
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        unsafe {
            let window_frame = NSWindow::frame(*self.window);
            Some((window_frame.size.width as u32, window_frame.size.height as u32))
        }
    }

    #[inline]
    pub fn set_inner_size(&self, width: u32, height: u32) {
        unsafe {
            NSWindow::setContentSize_(*self.window, NSSize::new(width as f64, height as f64));
        }
    }

    #[inline]
    pub fn platform_display(&self) -> *mut libc::c_void {
        unimplemented!()
    }

    #[inline]
    pub fn platform_window(&self) -> *mut libc::c_void {
        *self.window as *mut libc::c_void
    }

    pub fn set_cursor(&self, cursor: MouseCursor) {
        let cursor_name = match cursor {
            MouseCursor::Arrow | MouseCursor::Default => "arrowCursor",
            MouseCursor::Hand => "pointingHandCursor",
            MouseCursor::Grabbing | MouseCursor::Grab => "closedHandCursor",
            MouseCursor::Text => "IBeamCursor",
            MouseCursor::VerticalText => "IBeamCursorForVerticalLayout",
            MouseCursor::Copy => "dragCopyCursor",
            MouseCursor::Alias => "dragLinkCursor",
            MouseCursor::NotAllowed | MouseCursor::NoDrop => "operationNotAllowedCursor",
            MouseCursor::ContextMenu => "contextualMenuCursor",
            MouseCursor::Crosshair => "crosshairCursor",
            MouseCursor::EResize => "resizeRightCursor",
            MouseCursor::NResize => "resizeUpCursor",
            MouseCursor::WResize => "resizeLeftCursor",
            MouseCursor::SResize => "resizeDownCursor",
            MouseCursor::EwResize | MouseCursor::ColResize => "resizeLeftRightCursor",
            MouseCursor::NsResize | MouseCursor::RowResize => "resizeUpDownCursor",

            /// TODO: Find appropriate OSX cursors
            MouseCursor::NeResize | MouseCursor::NwResize |
            MouseCursor::SeResize | MouseCursor::SwResize |
            MouseCursor::NwseResize | MouseCursor::NeswResize |

            MouseCursor::Cell | MouseCursor::NoneCursor |
            MouseCursor::Wait | MouseCursor::Progress | MouseCursor::Help |
            MouseCursor::Move | MouseCursor::AllScroll | MouseCursor::ZoomIn |
            MouseCursor::ZoomOut => "arrowCursor",
        };
        let sel = Sel::register(cursor_name);
        let cls = Class::get("NSCursor").unwrap();
        unsafe {
            use objc::Message;
            let cursor: id = cls.send_message(sel, ()).unwrap();
            let _: () = msg_send![cursor, set];
        }
    }

    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        let cls = Class::get("NSCursor").unwrap();

        // TODO: Check for errors.
        match state {
            CursorState::Normal => {
                let _: () = unsafe { msg_send![cls, unhide] };
                let _: i32 = unsafe { CGAssociateMouseAndMouseCursorPosition(true) };
                Ok(())
            },
            CursorState::Hide => {
                let _: () = unsafe { msg_send![cls, hide] };
                Ok(())
            },
            CursorState::Grab => {
                let _: i32 = unsafe { CGAssociateMouseAndMouseCursorPosition(false) };
                Ok(())
            }
        }
    }

    #[inline]
    pub fn hidpi_factor(&self) -> f32 {
        unsafe {
            NSWindow::backingScaleFactor(*self.window) as f32
        }
    }

    #[inline]
    pub fn set_cursor_position(&self, x: i32, y: i32) -> Result<(), ()> {
        let (window_x, window_y) = self.get_position().unwrap_or((0, 0));
        let (cursor_x, cursor_y) = (window_x + x, window_y + y);

        unsafe {
            // TODO: Check for errors.
            let _ = CGWarpMouseCursorPosition(appkit::CGPoint {
                x: cursor_x as appkit::CGFloat,
                y: cursor_y as appkit::CGFloat,
            });
            let _ = CGAssociateMouseAndMouseCursorPosition(true);
        }

        Ok(())
    }
}

// Convert the `cocoa::base::id` associated with a window to a usize to use as a unique identifier
// for the window.
pub fn get_window_id(window_cocoa_id: cocoa::base::id) -> Id {
    Id(window_cocoa_id as *const objc::runtime::Object as usize)
}

unsafe fn nswindow_set_min_dimensions<V: NSWindow + Copy>(
    window: V, min_width: f64, min_height: f64)
{
    window.setMinSize_(NSSize {
        width: min_width,
        height: min_height,
    });
    // If necessary, resize the window to match constraint
    let mut current_rect = NSWindow::frame(window);
    if current_rect.size.width < min_width {
        current_rect.size.width = min_width;
        window.setFrame_display_(current_rect, 0)
    }
    if current_rect.size.height < min_height {
        // The origin point of a rectangle is at its bottom left in Cocoa. To
        // ensure the window's top-left point remains the same:
        current_rect.origin.y +=
            current_rect.size.height - min_height;

        current_rect.size.height = min_height;
        window.setFrame_display_(current_rect, 0)
    }
}

unsafe fn nswindow_set_max_dimensions<V: NSWindow + Copy>(
    window: V, max_width: f64, max_height: f64)
{
    window.setMaxSize_(NSSize {
        width: max_width,
        height: max_height,
    });
    // If necessary, resize the window to match constraint
    let mut current_rect = NSWindow::frame(window);
    if current_rect.size.width > max_width {
        current_rect.size.width = max_width;
        window.setFrame_display_(current_rect, 0)
    }
    if current_rect.size.height > max_height {
        // The origin point of a rectangle is at its bottom left in
        // Cocoa. To ensure the window's top-left point remains the
        // same:
        current_rect.origin.y +=
            current_rect.size.height - max_height;

        current_rect.size.height = max_height;
        window.setFrame_display_(current_rect, 0)
    }
}

pub struct IdRef(id);

impl IdRef {
    fn new(i: id) -> IdRef {
        IdRef(i)
    }

    #[allow(dead_code)]
    fn retain(i: id) -> IdRef {
        if i != nil {
            let _: id = unsafe { msg_send![i, retain] };
        }
        IdRef(i)
    }

    fn non_nil(self) -> Option<IdRef> {
        if self.0 == nil { None } else { Some(self) }
    }
}

impl Drop for IdRef {
    fn drop(&mut self) {
        if self.0 != nil {
            let _: () = unsafe { msg_send![self.0, release] };
        }
    }
}

impl Deref for IdRef {
    type Target = id;
    fn deref<'a>(&'a self) -> &'a id {
        &self.0
    }
}

impl Clone for IdRef {
    fn clone(&self) -> IdRef {
        if self.0 != nil {
            let _: id = unsafe { msg_send![self.0, retain] };
        }
        IdRef(self.0)
    }
}

// #[allow(non_snake_case, non_upper_case_globals)]
// unsafe fn NSEventToEvent(window: &Window, nsevent: id) -> Option<Event> {
//     if nsevent == nil { return None; }
// 
//     let event_type = nsevent.eventType();
//     appkit::NSApp().sendEvent_(if let appkit::NSKeyDown = event_type { nil } else { nsevent });
// 
//     match event_type {
//         appkit::NSLeftMouseDown         => { Some(Event::MouseInput(ElementState::Pressed, MouseButton::Left)) },
//         appkit::NSLeftMouseUp           => { Some(Event::MouseInput(ElementState::Released, MouseButton::Left)) },
//         appkit::NSRightMouseDown        => { Some(Event::MouseInput(ElementState::Pressed, MouseButton::Right)) },
//         appkit::NSRightMouseUp          => { Some(Event::MouseInput(ElementState::Released, MouseButton::Right)) },
//         appkit::NSOtherMouseDown        => { Some(Event::MouseInput(ElementState::Pressed, MouseButton::Middle)) },
//         appkit::NSOtherMouseUp          => { Some(Event::MouseInput(ElementState::Released, MouseButton::Middle)) },
//         appkit::NSMouseEntered          => { Some(Event::MouseEntered) },
//         appkit::NSMouseExited           => { Some(Event::MouseLeft) },
//         appkit::NSMouseMoved            |
//         appkit::NSLeftMouseDragged      |
//         appkit::NSOtherMouseDragged     |
//         appkit::NSRightMouseDragged     => {
//             let window_point = nsevent.locationInWindow();
//             let cWindow: id = msg_send![nsevent, window];
//             let view_point = if cWindow == nil {
//                 let window_rect = window.window.convertRectFromScreen_(NSRect::new(window_point, NSSize::new(0.0, 0.0)));
//                 window.view.convertPoint_fromView_(window_rect.origin, nil)
//             } else {
//                 window.view.convertPoint_fromView_(window_point, nil)
//             };
//             let view_rect = NSView::frame(*window.view);
//             let scale_factor = window.hidpi_factor();
// 
//             Some(Event::MouseMoved((scale_factor * view_point.x as f32) as i32,
//                                    (scale_factor * (view_rect.size.height - view_point.y) as f32) as i32))
//         },
//         appkit::NSKeyDown => {
//             let mut events = VecDeque::new();
//             let received_c_str = nsevent.characters().UTF8String();
//             let received_str = CStr::from_ptr(received_c_str);
//             for received_char in from_utf8(received_str.to_bytes()).unwrap().chars() {
//                 events.push_back(Event::ReceivedCharacter(received_char));
//             }
// 
//             let vkey =  to_virtual_key_code(NSEvent::keyCode(nsevent));
//             events.push_back(Event::KeyboardInput(ElementState::Pressed, NSEvent::keyCode(nsevent) as u8, vkey));
//             let event = events.pop_front();
//             window.delegate.state.pending_events.lock().unwrap().extend(events.into_iter());
//             event
//         },
//         appkit::NSKeyUp => {
//             let vkey =  to_virtual_key_code(NSEvent::keyCode(nsevent));
// 
//             Some(Event::KeyboardInput(ElementState::Released, NSEvent::keyCode(nsevent) as u8, vkey))
//         },
//         appkit::NSFlagsChanged => {
//             let mut events = VecDeque::new();
//             let shift_modifier = Window::modifier_event(nsevent, appkit::NSShiftKeyMask, events::VirtualKeyCode::LShift, SHIFT_PRESSED);
//             if shift_modifier.is_some() {
//                 SHIFT_PRESSED = !SHIFT_PRESSED;
//                 events.push_back(shift_modifier.unwrap());
//             }
//             let ctrl_modifier = Window::modifier_event(nsevent, appkit::NSControlKeyMask, events::VirtualKeyCode::LControl, CTRL_PRESSED);
//             if ctrl_modifier.is_some() {
//                 CTRL_PRESSED = !CTRL_PRESSED;
//                 events.push_back(ctrl_modifier.unwrap());
//             }
//             let win_modifier = Window::modifier_event(nsevent, appkit::NSCommandKeyMask, events::VirtualKeyCode::LWin, WIN_PRESSED);
//             if win_modifier.is_some() {
//                 WIN_PRESSED = !WIN_PRESSED;
//                 events.push_back(win_modifier.unwrap());
//             }
//             let alt_modifier = Window::modifier_event(nsevent, appkit::NSAlternateKeyMask, events::VirtualKeyCode::LAlt, ALT_PRESSED);
//             if alt_modifier.is_some() {
//                 ALT_PRESSED = !ALT_PRESSED;
//                 events.push_back(alt_modifier.unwrap());
//             }
//             let event = events.pop_front();
//             window.delegate.state.pending_events.lock().unwrap().extend(events.into_iter());
//             event
//         },
//         appkit::NSScrollWheel => {
//             use events::MouseScrollDelta::{LineDelta, PixelDelta};
//             let scale_factor = window.hidpi_factor();
//             let delta = if nsevent.hasPreciseScrollingDeltas() == YES {
//                 PixelDelta(scale_factor * nsevent.scrollingDeltaX() as f32,
//                            scale_factor * nsevent.scrollingDeltaY() as f32)
//             } else {
//                 LineDelta(scale_factor * nsevent.scrollingDeltaX() as f32,
//                           scale_factor * nsevent.scrollingDeltaY() as f32)
//             };
//             let phase = match nsevent.phase() {
//                 appkit::NSEventPhaseMayBegin | appkit::NSEventPhaseBegan => TouchPhase::Started,
//                 appkit::NSEventPhaseEnded => TouchPhase::Ended,
//                 _ => TouchPhase::Moved,
//             };
//             Some(Event::MouseWheel(delta, phase))
//         },
//         appkit::NSEventTypePressure => {
//             Some(Event::TouchpadPressure(nsevent.pressure(), nsevent.stage()))
//         },
//         appkit::NSApplicationDefined => {
//             match nsevent.subtype() {
//                 appkit::NSEventSubtype::NSApplicationActivatedEventType => { Some(Event::Awakened) }
//                 _ => { None }
//             }
//         },
//         _  => { None },
//     }
// }
// 
// pub fn to_virtual_key_code(code: u16) -> Option<events::VirtualKeyCode> {
//     Some(match code {
//         0x00 => events::VirtualKeyCode::A,
//         0x01 => events::VirtualKeyCode::S,
//         0x02 => events::VirtualKeyCode::D,
//         0x03 => events::VirtualKeyCode::F,
//         0x04 => events::VirtualKeyCode::H,
//         0x05 => events::VirtualKeyCode::G,
//         0x06 => events::VirtualKeyCode::Z,
//         0x07 => events::VirtualKeyCode::X,
//         0x08 => events::VirtualKeyCode::C,
//         0x09 => events::VirtualKeyCode::V,
//         //0x0a => World 1,
//         0x0b => events::VirtualKeyCode::B,
//         0x0c => events::VirtualKeyCode::Q,
//         0x0d => events::VirtualKeyCode::W,
//         0x0e => events::VirtualKeyCode::E,
//         0x0f => events::VirtualKeyCode::R,
//         0x10 => events::VirtualKeyCode::Y,
//         0x11 => events::VirtualKeyCode::T,
//         0x12 => events::VirtualKeyCode::Key1,
//         0x13 => events::VirtualKeyCode::Key2,
//         0x14 => events::VirtualKeyCode::Key3,
//         0x15 => events::VirtualKeyCode::Key4,
//         0x16 => events::VirtualKeyCode::Key6,
//         0x17 => events::VirtualKeyCode::Key5,
//         0x18 => events::VirtualKeyCode::Equals,
//         0x19 => events::VirtualKeyCode::Key9,
//         0x1a => events::VirtualKeyCode::Key7,
//         0x1b => events::VirtualKeyCode::Minus,
//         0x1c => events::VirtualKeyCode::Key8,
//         0x1d => events::VirtualKeyCode::Key0,
//         0x1e => events::VirtualKeyCode::RBracket,
//         0x1f => events::VirtualKeyCode::O,
//         0x20 => events::VirtualKeyCode::U,
//         0x21 => events::VirtualKeyCode::LBracket,
//         0x22 => events::VirtualKeyCode::I,
//         0x23 => events::VirtualKeyCode::P,
//         0x24 => events::VirtualKeyCode::Return,
//         0x25 => events::VirtualKeyCode::L,
//         0x26 => events::VirtualKeyCode::J,
//         0x27 => events::VirtualKeyCode::Apostrophe,
//         0x28 => events::VirtualKeyCode::K,
//         0x29 => events::VirtualKeyCode::Semicolon,
//         0x2a => events::VirtualKeyCode::Backslash,
//         0x2b => events::VirtualKeyCode::Comma,
//         0x2c => events::VirtualKeyCode::Slash,
//         0x2d => events::VirtualKeyCode::N,
//         0x2e => events::VirtualKeyCode::M,
//         0x2f => events::VirtualKeyCode::Period,
//         0x30 => events::VirtualKeyCode::Tab,
//         0x31 => events::VirtualKeyCode::Space,
//         0x32 => events::VirtualKeyCode::Grave,
//         0x33 => events::VirtualKeyCode::Back,
//         //0x34 => unkown,
//         0x35 => events::VirtualKeyCode::Escape,
//         0x36 => events::VirtualKeyCode::RWin,
//         0x37 => events::VirtualKeyCode::LWin,
//         0x38 => events::VirtualKeyCode::LShift,
//         //0x39 => Caps lock,
//         //0x3a => Left alt,
//         0x3b => events::VirtualKeyCode::LControl,
//         0x3c => events::VirtualKeyCode::RShift,
//         //0x3d => Right alt,
//         0x3e => events::VirtualKeyCode::RControl,
//         //0x3f => Fn key,
//         //0x40 => F17 Key,
//         0x41 => events::VirtualKeyCode::Decimal,
//         //0x42 -> unkown,
//         0x43 => events::VirtualKeyCode::Multiply,
//         //0x44 => unkown,
//         0x45 => events::VirtualKeyCode::Add,
//         //0x46 => unkown,
//         0x47 => events::VirtualKeyCode::Numlock,
//         //0x48 => KeypadClear,
//         0x49 => events::VirtualKeyCode::VolumeUp,
//         0x4a => events::VirtualKeyCode::VolumeDown,
//         0x4b => events::VirtualKeyCode::Divide,
//         0x4c => events::VirtualKeyCode::NumpadEnter,
//         //0x4d => unkown,
//         0x4e => events::VirtualKeyCode::Subtract,
//         //0x4f => F18 key,
//         //0x50 => F19 Key,
//         0x51 => events::VirtualKeyCode::NumpadEquals,
//         0x52 => events::VirtualKeyCode::Numpad0,
//         0x53 => events::VirtualKeyCode::Numpad1,
//         0x54 => events::VirtualKeyCode::Numpad2,
//         0x55 => events::VirtualKeyCode::Numpad3,
//         0x56 => events::VirtualKeyCode::Numpad4,
//         0x57 => events::VirtualKeyCode::Numpad5,
//         0x58 => events::VirtualKeyCode::Numpad6,
//         0x59 => events::VirtualKeyCode::Numpad7,
//         //0x5a => F20 Key,
//         0x5b => events::VirtualKeyCode::Numpad8,
//         0x5c => events::VirtualKeyCode::Numpad9,
//         //0x5d => unkown,
//         //0x5e => unkown,
//         //0x5f => unkown,
//         0x60 => events::VirtualKeyCode::F5,
//         0x61 => events::VirtualKeyCode::F6,
//         0x62 => events::VirtualKeyCode::F7,
//         0x63 => events::VirtualKeyCode::F3,
//         0x64 => events::VirtualKeyCode::F8,
//         0x65 => events::VirtualKeyCode::F9,
//         //0x66 => unkown,
//         0x67 => events::VirtualKeyCode::F11,
//         //0x68 => unkown,
//         0x69 => events::VirtualKeyCode::F13,
//         //0x6a => F16 Key,
//         0x6b => events::VirtualKeyCode::F14,
//         //0x6c => unkown,
//         0x6d => events::VirtualKeyCode::F10,
//         //0x6e => unkown,
//         0x6f => events::VirtualKeyCode::F12,
//         //0x70 => unkown,
//         0x71 => events::VirtualKeyCode::F15,
//         0x72 => events::VirtualKeyCode::Insert,
//         0x73 => events::VirtualKeyCode::Home,
//         0x74 => events::VirtualKeyCode::PageUp,
//         0x75 => events::VirtualKeyCode::Delete,
//         0x76 => events::VirtualKeyCode::F4,
//         0x77 => events::VirtualKeyCode::End,
//         0x78 => events::VirtualKeyCode::F2,
//         0x79 => events::VirtualKeyCode::PageDown,
//         0x7a => events::VirtualKeyCode::F1,
//         0x7b => events::VirtualKeyCode::Left,
//         0x7c => events::VirtualKeyCode::Right,
//         0x7d => events::VirtualKeyCode::Down,
//         0x7e => events::VirtualKeyCode::Up,
//         //0x7f =>  unkown,
// 
//         _ => return None,
//     })
// }
