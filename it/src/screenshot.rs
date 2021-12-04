use crate::backend::Instance;
use byteorder::{WriteBytesExt, LE};
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::atomic::Ordering::Relaxed;

const ENABLED: bool = true;

pub fn take_screenshot(instance: &dyn Instance) {
    if !ENABLED {
        log::info!("Taking screenshots is disabled");
        return;
    }
    instance.take_screenshot();
}

pub fn log_image(buf: &[u8], width: u32, height: u32) {
    crate::test::with_test_data(|td| {
        let id = td.next_image_id.fetch_add(1, Relaxed);
        let screenshots = td.test_dir.join("screenshots");
        std::fs::create_dir_all(&screenshots).unwrap();
        let file = format!("{:03}.bmp", id);
        let full_path = screenshots.join(&file);
        let rel_path = Path::new("screenshots").join(&file);
        let mut w = BufWriter::new(
            OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&full_path)
                .unwrap(),
        );
        let _ = w.write_u8(0x42);
        let _ = w.write_u8(0x4d);
        let _ = w.write_u32::<LE>(14 + 40 + buf.len() as u32);
        let _ = w.write_u32::<LE>(0);
        let _ = w.write_u32::<LE>(14 + 40);

        let _ = w.write_u32::<LE>(40);
        let _ = w.write_u32::<LE>(width);
        let _ = w.write_i32::<LE>(-(height as i32));
        let _ = w.write_u16::<LE>(1);
        let _ = w.write_u16::<LE>(32);
        let _ = w.write_u32::<LE>(0);
        let _ = w.write_u32::<LE>(0);
        let _ = w.write_u32::<LE>(0);
        let _ = w.write_u32::<LE>(0);
        let _ = w.write_u32::<LE>(0);
        let _ = w.write_u32::<LE>(0);

        let _ = w.write_all(buf);

        log::info!("Took screenshot {}", rel_path.display());
    })
}
