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
    mql: MediaQueryListHandle,
    last_scale: f64,
}

impl ScaleChangeDetectorInternal {
    fn new<F>(window: web_sys::Window, handler: F) -> Rc<RefCell<Self>>
    where
        F: 'static + FnMut(ScaleChangeArgs),
    {
        let current_scale = super::scale_factor(&window);
        Rc::new_cyclic(|weak_self| {
            let weak_self = weak_self.clone();
            let mql = Self::create_mql(&window, move |mql| {
                if let Some(rc_self) = weak_self.upgrade() {
                    Self::handler(rc_self, mql);
                }
            });

            RefCell::new(Self {
                window,
                callback: Box::new(handler),
                mql,
                last_scale: current_scale,
            })
        })
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
            super::scale_factor(window)
        );
        mql
    }

    fn handler(this: Rc<RefCell<Self>>, mql: &MediaQueryList) {
        let weak_self = Rc::downgrade(&this);
        let mut this = this.borrow_mut();
        let old_scale = this.last_scale;
        let new_scale = super::scale_factor(&this.window);

        // TODO: confirm/reproduce this problem, see:
        // <https://github.com/rust-windowing/winit/issues/2597>.
        // This should never happen, but if it does then apparently the scale factor didn't change.
        if mql.matches() {
            warn!(
                "media query tracking scale factor was triggered without a change:\n\
                Media Query: {}\n\
                Current Scale: {new_scale}",
                mql.media(),
            );
            return;
        }

        (this.callback)(ScaleChangeArgs {
            old_scale,
            new_scale,
        });

        let new_mql = Self::create_mql(&this.window, move |mql| {
            if let Some(rc_self) = weak_self.upgrade() {
                Self::handler(rc_self, mql);
            }
        });
        this.mql = new_mql;
        this.last_scale = new_scale;
    }
}
