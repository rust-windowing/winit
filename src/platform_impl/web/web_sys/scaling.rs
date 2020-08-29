use super::super::ScaleChangeArgs;

use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{prelude::Closure, JsCast};
use web_sys::{MediaQueryList, MediaQueryListEvent};

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
    closure: Option<Closure<dyn FnMut(MediaQueryListEvent)>>,
    mql: Option<MediaQueryList>,
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
            closure: None,
            mql: None,
            last_scale: current_scale,
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
        let current_scale = super::scale_factor();
        // This media query initially matches the current `devicePixelRatio`.
        // We add 0.0001 to the lower and upper bounds such that it won't fail
        // due to floating point precision limitations.
        let media_query = format!(
            "(min-resolution: {:.4}dppx) and (max-resolution: {:.4}dppx)",
            current_scale - 0.0001,
            current_scale + 0.0001,
        );
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

impl Drop for ScaleChangeDetectorInternal {
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
