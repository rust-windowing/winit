mod canvas;
mod document;
mod timeout;

pub use self::canvas::Canvas;
pub use self::document::Document;
pub use self::timeout::Timeout;

pub fn request_animation_frame<F>(f: F)
where
    F: Fn(),
{
}

pub fn throw(msg: &str) {
    wasm_bindgen::throw_str(msg);
}
