use crate::icon::{BadIcon, RgbaIcon};
use std::borrow::Cow;
use std::fmt;

impl RgbaIcon {
    fn into_web_icon(self) -> Result<WebIcon, BadIcon> {
        WebIcon::from_rgba(
            self.rgba,
            self.width,
            self.height,
            self.hotspot_x,
            self.hotspot_y,
        )
    }
}

#[derive(Clone)]
pub struct WebIcon {
    pub(crate) inner: Cow<'static, str>,
}

impl WebIcon {
    pub fn from_url(url: &str, hotspot_x: u32, hotspot_y: u32) -> Self {
        Self {
            inner: format!("url({}) {} {}, auto", url, hotspot_x, hotspot_y).into(),
        }
    }

    pub fn from_rgba(
        rgba: Vec<u8>,
        width: u32,
        height: u32,
        hotspot_x: u32,
        hotspot_y: u32,
    ) -> Result<Self, BadIcon> {
        let mut data = vec![];
        {
            let mut encoder = png::Encoder::new(&mut data, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&rgba).unwrap();
        }
        Ok(Self {
            inner: format!(
                "url(data:image/png;base64,{}) {} {}, auto",
                base64::encode(&data),
                hotspot_x,
                hotspot_y
            )
            .into(),
        })
    }
}

impl fmt::Debug for WebIcon {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        (*self.inner).fmt(formatter)
    }
}
