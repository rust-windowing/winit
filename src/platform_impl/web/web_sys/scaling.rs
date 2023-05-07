use super::super::ScaleChangeArgs;
use super::media_query_handle::MediaQueryListHandle;

use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::Closure;
use web_sys::MediaQueryListEvent;

pub struct ScaleChangeDetector(Rc<RefCell<ScaleChangeDetectorInternal>>);

impl ScaleChangeDetector {
    pub(crate) fn new<F>(handler: F) -> Self
    where
        F: 'static + FnMut(ScaleChangeArgs),
    {
        Self(ScaleChangeDetectorInternal::new(handler))
    }
}

/// This is a helper type to help manage the `MediaQueryList` used for detecting
/// changes of the `devicePixelRatio`.
struct ScaleChangeDetectorInternal {
    callback: Box<dyn FnMut(ScaleChangeArgs)>,
    mql: Option<MediaQueryListHandle>,
    last_scale: f64,
}

impl ScaleChangeDetectorInternal {
    fn new<F>(handler: F) -> Rc<RefCell<Self>>
    where
        F: 'static + FnMut(ScaleChangeArgs),
    {
        let current_scale = super::scale_factor();
        let new_self = Rc::new(RefCell::new(Self {
            callback: Box::new(handler),
            mql: None,
            last_scale: current_scale,
        }));

        let weak_self = Rc::downgrade(&new_self);
        let closure = Closure::wrap(Box::new(move |event: MediaQueryListEvent| {
            if let Some(rc_self) = weak_self.upgrade() {
                rc_self.borrow_mut().handler(event);
            }
        }) as Box<dyn FnMut(_)>);

        let mql = Self::create_mql(closure);
        {
            let mut borrowed_self = new_self.borrow_mut();
            borrowed_self.mql = mql;
        }
        new_self
    }

    fn create_mql(
        closure: Closure<dyn FnMut(MediaQueryListEvent)>,
    ) -> Option<MediaQueryListHandle> {
        let current_scale = super::scale_factor();
        // This media query initially matches the current `devicePixelRatio`.
        // We add 0.0001 to the lower and upper bounds such that it won't fail
        // due to floating point precision limitations.
        let media_query = format!(
            "(min-resolution: {min_scale:.4}dppx) and (max-resolution: {max_scale:.4}dppx),
             (-webkit-min-device-pixel-ratio: {min_scale:.4}) and (-webkit-max-device-pixel-ratio: {max_scale:.4})",
            min_scale = current_scale - 0.0001, max_scale= current_scale + 0.0001,
        );
        let mql = MediaQueryListHandle::new(&media_query, closure);
        if let Some(mql) = &mql {
            assert!(mql.mql().matches());
        }
        mql
    }

    fn handler(&mut self, _event: MediaQueryListEvent) {
        let mql = self
            .mql
            .take()
            .expect("DevicePixelRatioChangeDetector::mql should not be None");
        let closure = mql.remove();
        let new_scale = super::scale_factor();
        (self.callback)(ScaleChangeArgs {
            old_scale: self.last_scale,
            new_scale,
        });
        let new_mql = Self::create_mql(closure);
        self.mql = new_mql;
        self.last_scale = new_scale;
    }
}
