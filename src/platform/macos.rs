#![cfg(target_os = "macos")]

use std::{collections::hash_map::Entry, os::raw::c_void};

use cocoa::appkit::NSApp;
pub use objc;
use objc::{
    msg_send,
    rc::autoreleasepool,
    runtime::{Object, Sel},
    Encode,
};

use crate::{
    dpi::LogicalSize,
    event_loop::{EventLoop, EventLoopWindowTarget},
    monitor::MonitorHandle,
    platform_impl::{
        create_delegate_class, get_aux_state_mut, get_aux_state_ref,
        EventLoop as PlatformEventLoop, IdRef, BASE_APP_DELEGATE_METHODS,
    },
    window::{Window, WindowBuilder},
};

/// Additional methods on `Window` that are specific to MacOS.
pub trait WindowExtMacOS {
    /// Returns a pointer to the cocoa `NSWindow` that is used by this window.
    ///
    /// The pointer will become invalid when the `Window` is destroyed.
    fn ns_window(&self) -> *mut c_void;

    /// Returns a pointer to the cocoa `NSView` that is used by this window.
    ///
    /// The pointer will become invalid when the `Window` is destroyed.
    fn ns_view(&self) -> *mut c_void;

    /// Returns whether or not the window is in simple fullscreen mode.
    fn simple_fullscreen(&self) -> bool;

    /// Toggles a fullscreen mode that doesn't require a new macOS space.
    /// Returns a boolean indicating whether the transition was successful (this
    /// won't work if the window was already in the native fullscreen).
    ///
    /// This is how fullscreen used to work on macOS in versions before Lion.
    /// And allows the user to have a fullscreen window without using another
    /// space or taking control over the entire monitor.
    fn set_simple_fullscreen(&self, fullscreen: bool) -> bool;

    /// Returns whether or not the window has shadow.
    fn has_shadow(&self) -> bool;

    /// Sets whether or not the window has shadow.
    fn set_has_shadow(&self, has_shadow: bool);
}

impl WindowExtMacOS for Window {
    #[inline]
    fn ns_window(&self) -> *mut c_void {
        self.window.ns_window()
    }

    #[inline]
    fn ns_view(&self) -> *mut c_void {
        self.window.ns_view()
    }

    #[inline]
    fn simple_fullscreen(&self) -> bool {
        self.window.simple_fullscreen()
    }

    #[inline]
    fn set_simple_fullscreen(&self, fullscreen: bool) -> bool {
        self.window.set_simple_fullscreen(fullscreen)
    }

    #[inline]
    fn has_shadow(&self) -> bool {
        self.window.has_shadow()
    }

    #[inline]
    fn set_has_shadow(&self, has_shadow: bool) {
        self.window.set_has_shadow(has_shadow)
    }
}

/// Corresponds to `NSApplicationActivationPolicy`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActivationPolicy {
    /// Corresponds to `NSApplicationActivationPolicyRegular`.
    Regular,
    /// Corresponds to `NSApplicationActivationPolicyAccessory`.
    Accessory,
    /// Corresponds to `NSApplicationActivationPolicyProhibited`.
    Prohibited,
}

impl Default for ActivationPolicy {
    fn default() -> Self {
        ActivationPolicy::Regular
    }
}

/// Additional methods on `WindowBuilder` that are specific to MacOS.
///
/// **Note:** Properties dealing with the titlebar will be overwritten by the `with_decorations` method
/// on the base `WindowBuilder`:
///
///  - `with_titlebar_transparent`
///  - `with_title_hidden`
///  - `with_titlebar_hidden`
///  - `with_titlebar_buttons_hidden`
///  - `with_fullsize_content_view`
pub trait WindowBuilderExtMacOS {
    /// Enables click-and-drag behavior for the entire window, not just the titlebar.
    fn with_movable_by_window_background(self, movable_by_window_background: bool)
        -> WindowBuilder;
    /// Makes the titlebar transparent and allows the content to appear behind it.
    fn with_titlebar_transparent(self, titlebar_transparent: bool) -> WindowBuilder;
    /// Hides the window title.
    fn with_title_hidden(self, title_hidden: bool) -> WindowBuilder;
    /// Hides the window titlebar.
    fn with_titlebar_hidden(self, titlebar_hidden: bool) -> WindowBuilder;
    /// Hides the window titlebar buttons.
    fn with_titlebar_buttons_hidden(self, titlebar_buttons_hidden: bool) -> WindowBuilder;
    /// Makes the window content appear behind the titlebar.
    fn with_fullsize_content_view(self, fullsize_content_view: bool) -> WindowBuilder;
    /// Build window with `resizeIncrements` property. Values must not be 0.
    fn with_resize_increments(self, increments: LogicalSize<f64>) -> WindowBuilder;
    fn with_disallow_hidpi(self, disallow_hidpi: bool) -> WindowBuilder;
    fn with_has_shadow(self, has_shadow: bool) -> WindowBuilder;
}

impl WindowBuilderExtMacOS for WindowBuilder {
    #[inline]
    fn with_movable_by_window_background(
        mut self,
        movable_by_window_background: bool,
    ) -> WindowBuilder {
        self.platform_specific.movable_by_window_background = movable_by_window_background;
        self
    }

    #[inline]
    fn with_titlebar_transparent(mut self, titlebar_transparent: bool) -> WindowBuilder {
        self.platform_specific.titlebar_transparent = titlebar_transparent;
        self
    }

    #[inline]
    fn with_titlebar_hidden(mut self, titlebar_hidden: bool) -> WindowBuilder {
        self.platform_specific.titlebar_hidden = titlebar_hidden;
        self
    }

    #[inline]
    fn with_titlebar_buttons_hidden(mut self, titlebar_buttons_hidden: bool) -> WindowBuilder {
        self.platform_specific.titlebar_buttons_hidden = titlebar_buttons_hidden;
        self
    }

    #[inline]
    fn with_title_hidden(mut self, title_hidden: bool) -> WindowBuilder {
        self.platform_specific.title_hidden = title_hidden;
        self
    }

    #[inline]
    fn with_fullsize_content_view(mut self, fullsize_content_view: bool) -> WindowBuilder {
        self.platform_specific.fullsize_content_view = fullsize_content_view;
        self
    }

    #[inline]
    fn with_resize_increments(mut self, increments: LogicalSize<f64>) -> WindowBuilder {
        self.platform_specific.resize_increments = Some(increments.into());
        self
    }

    #[inline]
    fn with_disallow_hidpi(mut self, disallow_hidpi: bool) -> WindowBuilder {
        self.platform_specific.disallow_hidpi = disallow_hidpi;
        self
    }

    #[inline]
    fn with_has_shadow(mut self, has_shadow: bool) -> WindowBuilder {
        self.platform_specific.has_shadow = has_shadow;
        self
    }
}

pub trait DelegateMethod {
    fn register_method<T>(self, sel: Sel, el: &mut PlatformEventLoop<T>) -> Result<(), String>;
}
macro_rules! impl_delegate_method {
    ($($p:ident: $t:ident),*) => {
        // method_decl_impl!(-T, R, extern fn(&T, Sel $(, $t)*) -> R, $($t),*);
        impl<$($t, )* R> DelegateMethod for Box<dyn Fn($($t, )*) -> R + 'static>
        where
            $($t: Clone + Encode + 'static, )*
            R: Encode + 'static
        {
            fn register_method<T>(self, sel: Sel, el: &mut PlatformEventLoop<T>) -> Result<(), String> {

                // -------------------------------------------------------------------------
                // HANDLER
                // Allowing non-snake-case because we use the typename in the parameter name
                // `param_$t`
                #[allow(non_snake_case)]
                extern "C" fn method_handler<$($t, )* R>(this: &Object, sel: Sel, $($p: $t, )*) -> R
                where
                    $($t: Clone + 'static, )*
                    R: 'static,
                {
                    // Let's call the base winit handler first.
                    {
                        let guard = BASE_APP_DELEGATE_METHODS.read().unwrap();
                        if let Some(base_method) = guard.get(sel.name()) {
                            unsafe {
                                let base_method = std::mem::transmute::<
                                    unsafe extern fn(),
                                    extern fn(&Object, Sel, $($t, )*) -> R
                                >(*base_method);
                                base_method(this, sel, $($p.clone(), )*);
                            }
                        }
                    }
                    let mut retval: Option<R> = None;
                    let aux = unsafe { get_aux_state_ref(this) };
                    if let Some(callbacks) = aux.user_methods.get(sel.name()) {
                        // The `methods` is a `Vec<Box<Box<Fn(...)>>>`
                        for cb in callbacks.iter() {
                            // Could this be done with fewer indirections?
                            if let Some(cb) = cb.downcast_ref::<Box<dyn Fn($($t, )*) -> R>>() {
                                let v = (cb)($($p.clone(), )*);
                                if retval.is_none() {
                                    retval = Some(v);
                                }
                            } else {
                                warn!("Failed to downcast closure when handling {}", sel.name());
                            }
                        }
                    }
                    retval.expect(&format!(
                        "Couldn't get a return value during {:?}. This probably indicates that no appropriate callback was found", sel.name()
                    ))
                }
                // -------------------------------------------------------------------------

                let self_boxed = Box::new(self as Box<dyn Fn($($t, )*) -> R>);

                // println!("created delegate class {}", delegate_class.name());
                let mut delegate_state = unsafe {get_aux_state_mut(&mut **el.delegate)};
                match delegate_state.user_methods.entry(sel.name().to_string()) {
                    Entry::Occupied(mut e) => {
                        e.get_mut().push(self_boxed);
                    }
                    Entry::Vacant(e) => {
                        e.insert(vec![self_boxed]);

                        // This user method doesn't have a defined callback in the app delegate class yet,
                        // so let's create a new class for this method
                        unsafe {
                            let prev_delegate_class = (**el.delegate).class();
                            let mut decl = create_delegate_class(prev_delegate_class);
                            decl.add_method(
                                // sel!(application:openFiles:),
                                sel,
                                method_handler::<$($t, )* R> as extern "C" fn(&Object, Sel, $($t, )*) -> R,
                            );
                            let delegate_class = decl.register();
                            let new_delegate = IdRef::new(msg_send![delegate_class, new]);
                            let mut new_state = get_aux_state_mut(&mut **new_delegate);
                            std::mem::swap(&mut *new_state, &mut *delegate_state);
                            let app = NSApp();
                            autoreleasepool(|| {
                                let _: () = msg_send![app, setDelegate:*new_delegate];
                            });
                            el.delegate = new_delegate;
                        }
                    }
                }
                Ok(())
            }
        }
    }
}
impl_delegate_method!();
impl_delegate_method!(a: A);
impl_delegate_method!(a: A, b: B);
impl_delegate_method!(a: A, b: B, c: C);
impl_delegate_method!(a: A, b: B, c: C, d: D);
impl_delegate_method!(a: A, b: B, c: C, d: D, e: E);
impl_delegate_method!(a: A, b: B, c: C, d: D, e: E, f: F);
impl_delegate_method!(a: A, b: B, c: C, d: D, e: E, f: F, g: G);
impl_delegate_method!(a: A, b: B, c: C, d: D, e: E, f: F, g: G, h: H);

pub trait EventLoopExtMacOS {
    /// Sets the activation policy for the application. It is set to
    /// `NSApplicationActivationPolicyRegular` by default.
    ///
    /// This function only takes effect if it's called before calling [`run`](crate::event_loop::EventLoop::run) or
    /// [`run_return`](crate::platform::run_return::EventLoopExtRunReturn::run_return)
    fn set_activation_policy(&mut self, activation_policy: ActivationPolicy);

    /// Used to prevent a default menubar menu from getting created
    ///
    /// The default menu creation is enabled by default.
    ///
    /// This function only takes effect if it's called before calling
    /// [`run`](crate::event_loop::EventLoop::run) or
    /// [`run_return`](crate::platform::run_return::EventLoopExtRunReturn::run_return)
    fn enable_default_menu_creation(&mut self, enable: bool);

    /// Adds a new callback method for the application delegate.
    ///
    /// ### Safety
    /// As the underlying `add_method` documentation writes:
    /// > Unsafe because the caller must ensure that the types match those that are expected when the method is invoked from Objective-C.
    unsafe fn add_application_method<F: DelegateMethod>(
        &mut self,
        sel: Sel,
        method: F,
    ) -> Result<(), String>;
}
impl<T> EventLoopExtMacOS for EventLoop<T> {
    #[inline]
    fn set_activation_policy(&mut self, activation_policy: ActivationPolicy) {
        unsafe {
            get_aux_state_mut(&**self.event_loop.delegate).activation_policy = activation_policy;
        }
    }

    #[inline]
    fn enable_default_menu_creation(&mut self, enable: bool) {
        unsafe {
            get_aux_state_mut(&**self.event_loop.delegate).create_default_menu = enable;
        }
    }

    unsafe fn add_application_method<F: DelegateMethod>(
        &mut self,
        sel: Sel,
        method: F,
    ) -> Result<(), String> {
        method.register_method(sel, &mut self.event_loop)
    }
}

/// Additional methods on `MonitorHandle` that are specific to MacOS.
pub trait MonitorHandleExtMacOS {
    /// Returns the identifier of the monitor for Cocoa.
    fn native_id(&self) -> u32;
    /// Returns a pointer to the NSScreen representing this monitor.
    fn ns_screen(&self) -> Option<*mut c_void>;
}

impl MonitorHandleExtMacOS for MonitorHandle {
    #[inline]
    fn native_id(&self) -> u32 {
        self.inner.native_identifier()
    }

    fn ns_screen(&self) -> Option<*mut c_void> {
        self.inner.ns_screen().map(|s| s as *mut c_void)
    }
}

/// Additional methods on `EventLoopWindowTarget` that are specific to macOS.
pub trait EventLoopWindowTargetExtMacOS {
    /// Hide the entire application. In most applications this is typically triggered with Command-H.
    fn hide_application(&self);
    /// Hide the other applications. In most applications this is typically triggered with Command+Option-H.
    fn hide_other_applications(&self);
}

impl<T> EventLoopWindowTargetExtMacOS for EventLoopWindowTarget<T> {
    fn hide_application(&self) {
        self.p.hide_application()
    }

    fn hide_other_applications(&self) {
        self.p.hide_other_applications()
    }
}
