use web_sys::MediaQueryList;

use super::super::ScaleChangeArgs;
use super::media_query_handle::MediaQueryListHandle;

use std::{cell::RefCell, rc::Rc};

pub struct ScaleChangeDetector(Rc<RefCell<ScaleChangeDetectorInternal>>);

impl ScaleChangeDetector {
    pub(crate) fn new<F>(window: web_sys::Window, handler: F) -> Self
    where
        F: 'static + FnMut(ScaleChangeArgs),
    {
        Self(ScaleChangeDetectorInternal::new(window, handler))
    }
}

/// This is a helper type to help manage the `MediaQueryList` used for detecting
/// changes of the `devicePixelRatio`.
struct ScaleChangeDetectorInternal {
    window: web_sys::Window,
    callback: Box<dyn FnMut(ScaleChangeArgs)>,
    mql: Option<MediaQueryListHandle>,
    last_scale: f64,
}

impl ScaleChangeDetectorInternal {
    fn new<F>(window: web_sys::Window, handler: F) -> Rc<RefCell<Self>>
    where
        F: 'static + FnMut(ScaleChangeArgs),
    {
        let current_scale = super::scale_factor(&window);
        let new_self = Rc::new(RefCell::new(Self {
            window: window.clone(),
            callback: Box::new(handler),
            mql: None,
            last_scale: current_scale,
        }));

        let weak_self = Rc::downgrade(&new_self);
        let mql = Self::create_mql(&window, move |mql| {
            if let Some(rc_self) = weak_self.upgrade() {
                Self::handler(rc_self, mql);
            }
        });
        {
            let mut borrowed_self = new_self.borrow_mut();
            borrowed_self.mql = Some(mql);
        }
        new_self
    }

    fn create_mql<F>(window: &web_sys::Window, closure: F) -> MediaQueryListHandle
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
            super::scale_factor(window,)
        );
        mql
    }

    fn handler(this: Rc<RefCell<Self>>, mql: &MediaQueryList) {
        let weak_self = Rc::downgrade(&this);
        let mut this = this.borrow_mut();
        let old_scale = this.last_scale;
        let new_scale = super::scale_factor(&this.window);
        (this.callback)(ScaleChangeArgs {
            old_scale,
            new_scale,
        });

        // If this matches, then the scale factor is back to it's
        // old value again, so we won't need to update the query.
        if !mql.matches() {
            let new_mql = Self::create_mql(&this.window, move |mql| {
                if let Some(rc_self) = weak_self.upgrade() {
                    Self::handler(rc_self, mql);
                }
            });
            this.mql = Some(new_mql);
            this.last_scale = new_scale;
        }
    }
}
