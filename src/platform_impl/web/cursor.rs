use super::backend::Style;
use super::main_thread::{MainThreadMarker, MainThreadSafe};
use super::r#async::{AbortHandle, Abortable, DropAbortHandle, Notifier};
use super::EventLoopWindowTarget;
use crate::cursor::{BadImage, Cursor, CursorImage};
use cursor_icon::CursorIcon;
use std::ops::Deref;
use std::sync::Weak;
use std::{
    cell::RefCell,
    future,
    hash::{Hash, Hasher},
    mem,
    ops::DerefMut,
    rc::Rc,
    sync::Arc,
    task::{Poll, Waker},
};
use wasm_bindgen::{closure::Closure, JsCast};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Blob, Document, HtmlCanvasElement, HtmlImageElement, ImageBitmap, ImageBitmapOptions,
    ImageBitmapRenderingContext, ImageData, PremultiplyAlpha, Url, Window,
};

#[derive(Debug)]
pub(crate) enum CustomCursorBuilder {
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

#[derive(Clone, Debug)]
pub struct CustomCursor(Arc<MainThreadSafe<RefCell<ImageState>>>);

impl Hash for CustomCursor {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

impl PartialEq for CustomCursor {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for CustomCursor {}

impl CustomCursor {
    pub(crate) fn build<T>(
        builder: CustomCursorBuilder,
        window_target: &EventLoopWindowTarget<T>,
    ) -> Self {
        let main_thread = window_target.runner.main_thread();

        match builder {
            CustomCursorBuilder::Image(image) => ImageState::from_rgba(
                main_thread,
                window_target.runner.window(),
                window_target.runner.document().clone(),
                &image,
            ),
            CustomCursorBuilder::Url {
                url,
                hotspot_x,
                hotspot_y,
            } => ImageState::from_url(main_thread, url, hotspot_x, hotspot_y),
        }
    }
}

#[derive(Debug)]
pub struct CursorHandler(Rc<RefCell<Inner>>);

#[derive(Debug)]
struct Inner {
    main_thread: MainThreadMarker,
    style: Style,
    visible: bool,
    cursor: SelectedCursor,
}

impl CursorHandler {
    pub(crate) fn new(main_thread: MainThreadMarker, style: Style) -> Self {
        Self(Rc::new(RefCell::new(Inner {
            main_thread,
            style,
            visible: true,
            cursor: SelectedCursor::default(),
        })))
    }

    pub fn set_cursor(&self, cursor: Cursor) {
        let mut this = self.0.borrow_mut();

        match cursor {
            Cursor::Icon(icon) => {
                if let SelectedCursor::Icon(old_icon)
                | SelectedCursor::ImageLoading {
                    previous: Previous::Icon(old_icon),
                    ..
                } = &this.cursor
                {
                    if *old_icon == icon {
                        return;
                    }
                }

                this.cursor = SelectedCursor::Icon(icon);
                this.set_style();
            }
            Cursor::Custom(cursor) => {
                let cursor = cursor.inner;

                if let SelectedCursor::ImageLoading {
                    cursor: old_cursor, ..
                }
                | SelectedCursor::ImageReady(old_cursor) = &this.cursor
                {
                    if *old_cursor == cursor {
                        return;
                    }
                }

                let state = cursor.0.get(this.main_thread).borrow();

                match state.deref() {
                    ImageState::Loading { notifier, .. } => {
                        let notified = notifier.notified();
                        let handle = DropAbortHandle::new(AbortHandle::new());
                        let task = Abortable::new(handle.handle(), {
                            let weak = Rc::downgrade(&self.0);
                            async move {
                                notified.await;
                                let handler = weak
                                    .upgrade()
                                    .expect("`CursorHandler` invalidated without aborting");
                                handler.borrow_mut().notify();
                            }
                        });
                        wasm_bindgen_futures::spawn_local(async move {
                            let _ = task.await;
                        });

                        drop(state);
                        this.cursor = SelectedCursor::ImageLoading {
                            cursor,
                            previous: mem::take(&mut this.cursor).into(),
                            _handle: handle,
                        };
                    }
                    ImageState::Failed => log::error!("tried to load invalid cursor"),
                    ImageState::Ready { .. } => {
                        drop(state);
                        this.cursor = SelectedCursor::ImageReady(cursor);
                        this.set_style();
                    }
                };
            }
        }
    }

    pub fn set_cursor_visible(&self, visible: bool) {
        let mut this = self.0.borrow_mut();

        if !visible && this.visible {
            this.visible = false;
            this.style.set("cursor", "none");
        } else if visible && !this.visible {
            this.visible = true;
            this.set_style();
        }
    }
}

impl Inner {
    fn set_style(&self) {
        if self.visible {
            match &self.cursor {
                SelectedCursor::Icon(icon)
                | SelectedCursor::ImageLoading {
                    previous: Previous::Icon(icon),
                    ..
                } => self.style.set("cursor", icon.name()),
                SelectedCursor::ImageLoading {
                    previous: Previous::Image(cursor),
                    ..
                }
                | SelectedCursor::ImageReady(cursor) => {
                    if let ImageState::Ready { style, .. } =
                        cursor.0.get(self.main_thread).borrow().deref()
                    {
                        self.style.set("cursor", style)
                    } else {
                        unreachable!("found invalid saved state")
                    }
                }
            }
        }
    }

    fn notify(&mut self) {
        let SelectedCursor::ImageLoading {
            cursor, previous, ..
        } = mem::take(&mut self.cursor)
        else {
            unreachable!("found wrong state")
        };

        let state = cursor.0.get(self.main_thread).borrow();
        match state.deref() {
            ImageState::Failed => self.cursor = previous.into(),
            ImageState::Ready { .. } => {
                drop(state);
                self.cursor = SelectedCursor::ImageReady(cursor);
                self.set_style();
            }
            ImageState::Loading { .. } => unreachable!("notified without being ready"),
        }
    }
}

#[derive(Debug)]
enum SelectedCursor {
    Icon(CursorIcon),
    ImageLoading {
        cursor: CustomCursor,
        previous: Previous,
        _handle: DropAbortHandle,
    },
    ImageReady(CustomCursor),
}

impl Default for SelectedCursor {
    fn default() -> Self {
        Self::Icon(Default::default())
    }
}

impl From<Previous> for SelectedCursor {
    fn from(previous: Previous) -> Self {
        match previous {
            Previous::Icon(icon) => Self::Icon(icon),
            Previous::Image(cursor) => Self::ImageReady(cursor),
        }
    }
}

#[derive(Debug)]
pub enum Previous {
    Icon(CursorIcon),
    Image(CustomCursor),
}

impl From<SelectedCursor> for Previous {
    fn from(value: SelectedCursor) -> Self {
        match value {
            SelectedCursor::Icon(icon) => Self::Icon(icon),
            SelectedCursor::ImageLoading { previous, .. } => previous,
            SelectedCursor::ImageReady(image) => Self::Image(image),
        }
    }
}

#[derive(Debug)]
enum ImageState {
    Loading {
        notifier: Notifier,
        _handle: DropAbortHandle,
    },
    Failed,
    Ready {
        style: String,
        _object_url: Option<ObjectUrl>,
        _image: HtmlImageElement,
    },
}

impl ImageState {
    fn from_rgba(
        main_thread: MainThreadMarker,
        window: &Window,
        document: Document,
        image: &CursorImage,
    ) -> CustomCursor {
        // 1. Create an `ImageData` from the RGBA data.
        // 2. Create an `ImageBitmap` from the `ImageData`.
        // 3. Draw `ImageBitmap` on an `HTMLCanvasElement`.
        // 4. Create a `Blob` from the `HTMLCanvasElement`.
        // 5. Create an object URL from the `Blob`.
        // 6. Decode the image on an `HTMLImageElement` from the URL.
        // 7. Notify listeners.

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

        let handle = AbortHandle::new();
        let this = CustomCursor(Arc::new(MainThreadSafe::new(
            main_thread,
            RefCell::new(ImageState::Loading {
                notifier: Notifier::new(),
                _handle: DropAbortHandle::new(handle.clone()),
            }),
        )));

        let task = Abortable::new(handle, {
            let weak = Arc::downgrade(&this.0);
            let CursorImage {
                width,
                height,
                hotspot_x,
                hotspot_y,
                ..
            } = *image;
            async move {
                let bitmap: ImageBitmap = bitmap
                    .await
                    .expect("found invalid state in `ImageData`")
                    .unchecked_into();

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
                drop(bitmap);
                drop(context);

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
                drop(canvas);

                let Some(blob) = blob else {
                    log::error!("creating object URL from custom cursor failed");
                    let this = weak
                        .upgrade()
                        .expect("`CursorHandler` invalidated without aborting");
                    let mut this = this.get(main_thread).borrow_mut();
                    let ImageState::Loading { notifier, .. } =
                        mem::replace(this.deref_mut(), ImageState::Failed)
                    else {
                        unreachable!("found invalid state");
                    };
                    notifier.notify();

                    return;
                };

                // 5. Create an object URL from the `Blob`.
                let url = Url::create_object_url_with_blob(&blob)
                    .expect("unexpected exception in `URL.createObjectURL()`");
                let url = UrlType::Object(ObjectUrl(url));

                Self::decode(main_thread, weak, url, hotspot_x, hotspot_y).await;
            }
        });

        wasm_bindgen_futures::spawn_local(async move {
            let _ = task.await;
        });

        this
    }

    fn from_url(
        main_thread: MainThreadMarker,
        url: String,
        hotspot_x: u16,
        hotspot_y: u16,
    ) -> CustomCursor {
        let handle = AbortHandle::new();
        let this = CustomCursor(Arc::new(MainThreadSafe::new(
            main_thread,
            RefCell::new(ImageState::Loading {
                notifier: Notifier::new(),
                _handle: DropAbortHandle::new(handle.clone()),
            }),
        )));

        let task = Abortable::new(
            handle,
            Self::decode(
                main_thread,
                Arc::downgrade(&this.0),
                UrlType::Plain(url),
                hotspot_x,
                hotspot_y,
            ),
        );
        wasm_bindgen_futures::spawn_local(async move {
            let _ = task.await;
        });

        this
    }

    async fn decode(
        main_thread: MainThreadMarker,
        weak: Weak<MainThreadSafe<RefCell<ImageState>>>,
        url: UrlType,
        hotspot_x: u16,
        hotspot_y: u16,
    ) {
        // 6. Decode the image on an `HTMLImageElement` from the URL.
        let image =
            HtmlImageElement::new().expect("unexpected exception in `new HtmlImageElement`");
        image.set_src(url.url());
        let result = JsFuture::from(image.decode()).await;

        let this = weak
            .upgrade()
            .expect("`CursorHandler` invalidated without aborting");
        let mut this = this.get(main_thread).borrow_mut();

        if let Err(error) = result {
            log::error!("decoding custom cursor failed: {error:?}");
            let ImageState::Loading { notifier, .. } =
                mem::replace(this.deref_mut(), ImageState::Failed)
            else {
                unreachable!("found invalid state");
            };
            notifier.notify();

            return;
        }

        let ImageState::Loading { notifier, .. } = mem::replace(
            this.deref_mut(),
            ImageState::Ready {
                style: format!("url({}) {hotspot_x} {hotspot_y}, auto", url.url()),
                _object_url: match url {
                    UrlType::Plain(_) => None,
                    UrlType::Object(object_url) => Some(object_url),
                },
                _image: image,
            },
        ) else {
            unreachable!("found invalid state");
        };

        // 7. Notify listeners.
        notifier.notify();
    }
}

enum UrlType {
    Plain(String),
    Object(ObjectUrl),
}

impl UrlType {
    fn url(&self) -> &str {
        match &self {
            UrlType::Plain(url) => url,
            UrlType::Object(object_url) => &object_url.0,
        }
    }
}

#[derive(Debug)]
struct ObjectUrl(String);

impl Drop for ObjectUrl {
    fn drop(&mut self) {
        Url::revoke_object_url(&self.0).expect("unexpected exception in `URL.revokeObjectURL()`");
    }
}
