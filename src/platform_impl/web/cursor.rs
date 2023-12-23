use std::{
    cell::RefCell,
    future, mem,
    ops::DerefMut,
    rc::{self, Rc},
    sync::{self, Arc},
    task::{Poll, Waker},
};

use crate::{
    cursor::{BadImage, CursorImage},
    platform_impl::platform::r#async,
};
use cursor_icon::CursorIcon;
use once_cell::sync::Lazy;
use wasm_bindgen::{closure::Closure, JsCast};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Blob, Document, HtmlCanvasElement, HtmlImageElement, ImageBitmap, ImageBitmapOptions,
    ImageBitmapRenderingContext, ImageData, PremultiplyAlpha, Url, Window,
};

use self::thread_safe::ThreadSafe;

use super::{backend::Style, r#async::AsyncSender, EventLoopWindowTarget};

#[derive(Debug)]
pub enum CustomCursorBuilder {
    Image(CursorImage),
    Url {
        url: String,
        hotspot_x: u16,
        hotspot_y: u16,
    },
}

impl CustomCursorBuilder {
    pub fn from_rgba(
        rgba: Vec<u8>,
        width: u16,
        height: u16,
        hotspot_x: u16,
        hotspot_y: u16,
    ) -> Result<CustomCursorBuilder, BadImage> {
        Ok(CustomCursorBuilder::Image(CursorImage::from_rgba(
            rgba, width, height, hotspot_x, hotspot_y,
        )?))
    }
}

#[derive(Debug)]
pub struct CustomCursor(Option<ThreadSafe<RefCell<ImageState>>>);

static DROP_HANDLER: Lazy<AsyncSender<ThreadSafe<RefCell<ImageState>>>> = Lazy::new(|| {
    let (sender, receiver) = r#async::channel();
    wasm_bindgen_futures::spawn_local(async move { while receiver.next().await.is_ok() {} });

    sender
});

impl CustomCursor {
    fn new() -> Arc<Self> {
        Arc::new(Self(Some(ThreadSafe::new(RefCell::new(
            ImageState::Loading(None),
        )))))
    }

    fn get(&self) -> &RefCell<ImageState> {
        self.0
            .as_ref()
            .expect("value has accidently already been dropped")
            .get()
    }

    pub fn build<T>(
        builder: CustomCursorBuilder,
        window_target: &EventLoopWindowTarget<T>,
    ) -> Arc<CustomCursor> {
        Lazy::force(&DROP_HANDLER);

        match builder {
            CustomCursorBuilder::Image(image) => ImageState::from_rgba(
                window_target.runner.window(),
                window_target.runner.document().clone(),
                &image,
            ),
            CustomCursorBuilder::Url {
                url,
                hotspot_x,
                hotspot_y,
            } => ImageState::from_url(url, hotspot_x, hotspot_y),
        }
    }
}

impl Drop for CustomCursor {
    fn drop(&mut self) {
        let value = self
            .0
            .take()
            .expect("value has accidently already been dropped");

        if !value.in_origin_thread() {
            DROP_HANDLER
                .send(value)
                .expect("sender dropped in main thread")
        }
    }
}

#[derive(Debug)]
pub struct CursorState(Rc<RefCell<State>>);

impl CursorState {
    pub fn new(style: Style) -> Self {
        Self(Rc::new(RefCell::new(State {
            style,
            visible: true,
            cursor: SelectedCursor::default(),
        })))
    }

    pub fn set_cursor_icon(&self, cursor: CursorIcon) {
        let mut this = self.0.borrow_mut();

        if let SelectedCursor::ImageLoading { state, .. } = &this.cursor {
            if let ImageState::Loading(state) = state.get().borrow_mut().deref_mut() {
                state.take();
            }
        }

        this.cursor = SelectedCursor::Named(cursor);
        this.set_style();
    }

    pub fn set_custom_cursor(&self, cursor: Arc<CustomCursor>) {
        let mut this = self.0.borrow_mut();

        match cursor.get().borrow_mut().deref_mut() {
            ImageState::Loading(state) => {
                this.cursor = SelectedCursor::ImageLoading {
                    state: cursor.clone(),
                    previous: mem::take(&mut this.cursor).into(),
                };
                *state = Some(Rc::downgrade(&self.0));
            }
            ImageState::Failed => log::error!("tried to load invalid cursor"),
            ImageState::Ready(image) => {
                this.cursor = SelectedCursor::ImageReady(image.clone());
                this.set_style();
            }
        }
    }

    pub fn set_cursor_visible(&self, visible: bool) {
        let mut state = self.0.borrow_mut();

        if !visible && state.visible {
            state.visible = false;
            state.style.set("cursor", "none");
        } else if visible && !state.visible {
            state.visible = true;
            state.set_style();
        }
    }
}

#[derive(Debug)]
struct State {
    style: Style,
    visible: bool,
    cursor: SelectedCursor,
}

impl State {
    pub fn set_style(&self) {
        if self.visible {
            let value = match &self.cursor {
                SelectedCursor::Named(icon) => icon.name(),
                SelectedCursor::ImageLoading { previous, .. } => previous.style(),
                SelectedCursor::ImageReady(image) => &image.style,
            };

            self.style.set("cursor", value);
        }
    }
}

#[derive(Debug)]
enum SelectedCursor {
    Named(CursorIcon),
    ImageLoading {
        state: Arc<CustomCursor>,
        previous: Previous,
    },
    ImageReady(Rc<Image>),
}

impl Default for SelectedCursor {
    fn default() -> Self {
        Self::Named(Default::default())
    }
}

impl From<Previous> for SelectedCursor {
    fn from(previous: Previous) -> Self {
        match previous {
            Previous::Named(icon) => Self::Named(icon),
            Previous::Image(image) => Self::ImageReady(image),
        }
    }
}

#[derive(Debug)]
pub enum Previous {
    Named(CursorIcon),
    Image(Rc<Image>),
}

impl Previous {
    fn style(&self) -> &str {
        match self {
            Previous::Named(icon) => icon.name(),
            Previous::Image(image) => &image.style,
        }
    }
}

impl From<SelectedCursor> for Previous {
    fn from(value: SelectedCursor) -> Self {
        match value {
            SelectedCursor::Named(icon) => Self::Named(icon),
            SelectedCursor::ImageLoading { previous, .. } => previous,
            SelectedCursor::ImageReady(image) => Self::Image(image),
        }
    }
}

#[derive(Debug)]
enum ImageState {
    Loading(Option<rc::Weak<RefCell<State>>>),
    Failed,
    Ready(Rc<Image>),
}

impl ImageState {
    fn from_rgba(window: &Window, document: Document, image: &CursorImage) -> Arc<CustomCursor> {
        // 1. Create an `ImageData` from the RGBA data.
        // 2. Create an `ImageBitmap` from the `ImageData`.
        // 3. Draw `ImageBitmap` on an `HTMLCanvasElement`.
        // 4. Create a `Blob` from the `HTMLCanvasElement`.
        // 5. Create an object URL from the `Blob`.
        // 6. Decode the image on an `HTMLImageElement` from the URL.
        // 7. Change the `CursorState` if queued.

        // 1. Create an `ImageData` from the RGBA data.
        // Adapted from https://github.com/rust-windowing/softbuffer/blob/ab7688e2ed2e2eca51b3c4e1863a5bd7fe85800e/src/web.rs#L196-L223
        #[cfg(target_feature = "atomics")]
        // Can't share `SharedArrayBuffer` with `ImageData`.
        let result = {
            use js_sys::{Uint8Array, Uint8ClampedArray};
            use wasm_bindgen::prelude::wasm_bindgen;
            use wasm_bindgen::JsValue;

            #[wasm_bindgen]
            extern "C" {
                #[wasm_bindgen(js_namespace = ImageData)]
                type ImageDataExt;
                #[wasm_bindgen(catch, constructor, js_class = ImageData)]
                fn new(array: Uint8ClampedArray, sw: u32) -> Result<ImageDataExt, JsValue>;
            }

            let array = Uint8Array::new_with_length(image.rgba.len() as u32);
            array.copy_from(&image.rgba);
            let array = Uint8ClampedArray::new(&array);
            ImageDataExt::new(array, image.width as u32)
                .map(JsValue::from)
                .map(ImageData::unchecked_from_js)
        };
        #[cfg(not(target_feature = "atomics"))]
        let result = ImageData::new_with_u8_clamped_array(
            wasm_bindgen::Clamped(&image.rgba),
            image.width as u32,
        );
        let image_data = result.expect("found wrong image size");

        // 2. Create an `ImageBitmap` from the `ImageData`.
        //
        // We call `createImageBitmap()` before spawning the future,
        // to not have to clone the image buffer.
        let mut options = ImageBitmapOptions::new();
        options.premultiply_alpha(PremultiplyAlpha::None);
        let bitmap = JsFuture::from(
            window
                .create_image_bitmap_with_image_data_and_image_bitmap_options(&image_data, &options)
                .expect("unexpected exception in `createImageBitmap()`"),
        );

        let this = CustomCursor::new();

        wasm_bindgen_futures::spawn_local({
            let weak = Arc::downgrade(&this);
            let CursorImage {
                width,
                height,
                hotspot_x,
                hotspot_y,
                ..
            } = *image;
            async move {
                // Keep checking if all references are dropped between every `await` call.
                if weak.strong_count() == 0 {
                    return;
                }

                let bitmap: ImageBitmap = bitmap
                    .await
                    .expect("found invalid state in `ImageData`")
                    .unchecked_into();

                if weak.strong_count() == 0 {
                    return;
                }

                let canvas: HtmlCanvasElement = document
                    .create_element("canvas")
                    .expect("invalid tag name")
                    .unchecked_into();
                #[allow(clippy::disallowed_methods)]
                canvas.set_width(width as u32);
                #[allow(clippy::disallowed_methods)]
                canvas.set_height(height as u32);

                // 3. Draw `ImageBitmap` on an `HTMLCanvasElement`.
                let context: ImageBitmapRenderingContext = canvas
                    .get_context("bitmaprenderer")
                    .expect("unexpected exception in `HTMLCanvasElement.getContext()`")
                    .expect("`bitmaprenderer` context unsupported")
                    .unchecked_into();
                context.transfer_from_image_bitmap(&bitmap);

                // 4. Create a `Blob` from the `HTMLCanvasElement`.
                //
                // To keep the `Closure` alive until `HTMLCanvasElement.toBlob()` is done,
                // we do the whole `Waker` strategy. Commonly on `Drop` the callback is aborted,
                // but it would increase complexity and isn't possible in this case.
                // Keep in mind that `HTMLCanvasElement.toBlob()` can call the callback immediately.
                let value = Rc::new(RefCell::new(None));
                let waker = Rc::new(RefCell::<Option<Waker>>::new(None));
                let callback = Closure::once({
                    let value = value.clone();
                    let waker = waker.clone();
                    move |blob: Option<Blob>| {
                        *value.borrow_mut() = Some(blob);
                        if let Some(waker) = waker.borrow_mut().take() {
                            waker.wake();
                        }
                    }
                });
                canvas
                    .to_blob(callback.as_ref().unchecked_ref())
                    .expect("failed with `SecurityError` despite only source coming from memory");
                let blob = future::poll_fn(|cx| {
                    if let Some(blob) = value.borrow_mut().take() {
                        Poll::Ready(blob)
                    } else {
                        *waker.borrow_mut() = Some(cx.waker().clone());
                        Poll::Pending
                    }
                })
                .await;

                let url = {
                    let Some(this) = weak.upgrade() else {
                        return;
                    };
                    let mut this = this.get().borrow_mut();

                    let Some(blob) = blob else {
                        log::error!("creating custom cursor failed");
                        let ImageState::Loading(state) = this.deref_mut() else {
                            unreachable!("found invalid state");
                        };
                        let state = state.take();
                        *this = ImageState::Failed;

                        if let Some(state) = state.and_then(|weak| weak.upgrade()) {
                            let mut state = state.borrow_mut();
                            let SelectedCursor::ImageLoading { previous, .. } =
                                mem::take(&mut state.cursor)
                            else {
                                unreachable!("found invalid state");
                            };
                            state.cursor = previous.into();
                        }

                        return;
                    };

                    // 5. Create an object URL from the `Blob`.
                    Url::create_object_url_with_blob(&blob)
                        .expect("unexpected exception in `URL.createObjectURL()`")
                };

                Self::decode(weak, url, true, hotspot_x, hotspot_y).await;
            }
        });

        this
    }

    fn from_url(url: String, hotspot_x: u16, hotspot_y: u16) -> Arc<CustomCursor> {
        let this = CustomCursor::new();
        wasm_bindgen_futures::spawn_local(Self::decode(
            Arc::downgrade(&this),
            url,
            false,
            hotspot_x,
            hotspot_y,
        ));

        this
    }

    async fn decode(
        weak: sync::Weak<CustomCursor>,
        url: String,
        object: bool,
        hotspot_x: u16,
        hotspot_y: u16,
    ) {
        if weak.strong_count() == 0 {
            return;
        }

        // 6. Decode the image on an `HTMLImageElement` from the URL.
        let image =
            HtmlImageElement::new().expect("unexpected exception in `new HtmlImageElement`");
        image.set_src(&url);
        let result = JsFuture::from(image.decode()).await;

        let Some(this) = weak.upgrade() else {
            return;
        };
        let mut this = this.get().borrow_mut();

        let ImageState::Loading(state) = this.deref_mut() else {
            unreachable!("found invalid state");
        };
        let state = state.take();

        if let Err(error) = result {
            log::error!("creating custom cursor failed: {error:?}");
            *this = ImageState::Failed;

            if let Some(state) = state.and_then(|weak| weak.upgrade()) {
                let mut state = state.borrow_mut();
                let SelectedCursor::ImageLoading { previous, .. } = mem::take(&mut state.cursor)
                else {
                    unreachable!("found invalid state");
                };
                state.cursor = previous.into();
            }

            return;
        }

        let image = Image::new(url, object, image, hotspot_x, hotspot_y);

        // 7. Change the `CursorState` if queued.
        if let Some(state) = state.and_then(|weak| weak.upgrade()) {
            let mut state = state.borrow_mut();
            state.cursor = SelectedCursor::ImageReady(image.clone());
            state.set_style();
        }

        *this = ImageState::Ready(image);
    }
}

#[derive(Debug)]
pub struct Image {
    style: String,
    url: String,
    object: bool,
    _image: HtmlImageElement,
}

impl Drop for Image {
    fn drop(&mut self) {
        if self.object {
            Url::revoke_object_url(&self.url)
                .expect("unexpected exception in `URL.revokeObjectURL()`");
        }
    }
}

impl Image {
    fn new(
        url: String,
        object: bool,
        image: HtmlImageElement,
        hotspot_x: u16,
        hotspot_y: u16,
    ) -> Rc<Self> {
        let style = format!("url({url}) {hotspot_x} {hotspot_y}, auto");

        Rc::new(Self {
            style,
            url,
            object,
            _image: image,
        })
    }
}

mod thread_safe {
    use std::mem;
    use std::thread::{self, ThreadId};

    #[derive(Debug)]
    pub struct ThreadSafe<T> {
        origin_thread: ThreadId,
        value: T,
    }

    impl<T> ThreadSafe<T> {
        pub fn new(value: T) -> Self {
            Self {
                origin_thread: thread::current().id(),
                value,
            }
        }

        pub fn get(&self) -> &T {
            if self.origin_thread == thread::current().id() {
                &self.value
            } else {
                panic!("value not accessible outside its origin thread")
            }
        }

        pub fn in_origin_thread(&self) -> bool {
            self.origin_thread == thread::current().id()
        }
    }

    impl<T> Drop for ThreadSafe<T> {
        fn drop(&mut self) {
            if mem::needs_drop::<T>() && self.origin_thread != thread::current().id() {
                panic!("value can't be dropped outside its origin thread")
            }
        }
    }

    unsafe impl<T> Send for ThreadSafe<T> {}
    unsafe impl<T> Sync for ThreadSafe<T> {}
}
