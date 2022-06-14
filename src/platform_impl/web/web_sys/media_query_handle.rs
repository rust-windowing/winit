use wasm_bindgen::{prelude::Closure, JsCast};
use web_sys::{MediaQueryList, MediaQueryListEvent};

pub(super) struct MediaQueryListHandle {
    mql: MediaQueryList,
    listener: Option<Closure<dyn FnMut(MediaQueryListEvent)>>,
}

impl MediaQueryListHandle {
    pub fn new(
        media_query: &str,
        listener: Closure<dyn FnMut(MediaQueryListEvent)>,
    ) -> Option<Self> {
        let window = web_sys::window().expect("Failed to obtain window");
        let mql = window
            .match_media(media_query)
            .ok()
            .flatten()
            .and_then(|mql| {
                mql.add_listener_with_opt_callback(Some(listener.as_ref().unchecked_ref()))
                    .map(|_| mql)
                    .ok()
            });
        mql.map(|mql| Self {
            mql,
            listener: Some(listener),
        })
    }

    pub fn mql(&self) -> &MediaQueryList {
        &self.mql
    }

    /// Removes the listener and returns the original listener closure, which
    /// can be reused.
    pub fn remove(mut self) -> Closure<dyn FnMut(MediaQueryListEvent)> {
        let listener = self.listener.take().unwrap_or_else(|| unreachable!());
        remove_listener(&self.mql, &listener);
        listener
    }
}

impl Drop for MediaQueryListHandle {
    fn drop(&mut self) {
        if let Some(listener) = self.listener.take() {
            remove_listener(&self.mql, &listener);
        }
    }
}

fn remove_listener(mql: &MediaQueryList, listener: &Closure<dyn FnMut(MediaQueryListEvent)>) {
    mql.remove_listener_with_opt_callback(Some(listener.as_ref().unchecked_ref()))
        .unwrap_or_else(|e| {
            web_sys::console::error_2(&"Error removing media query listener".into(), &e)
        });
}
