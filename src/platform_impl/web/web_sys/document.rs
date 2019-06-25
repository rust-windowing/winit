pub struct Document;

impl Document {
    pub fn set_title(title: &str) {}

    pub fn on_blur<F>(f: F) {}

    pub fn on_focus<F>(f: F) {}

    pub fn on_key_up<F>(f: F) {}

    pub fn on_key_down<F>(f: F) {}
}
