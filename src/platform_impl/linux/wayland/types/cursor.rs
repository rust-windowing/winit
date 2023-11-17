use std::{fs::File, io::Write, sync::Arc};

use cursor_icon::CursorIcon;
use rustix::fd::{AsFd, OwnedFd};
use wayland_backend::client::ObjectData;
use wayland_client::{
    protocol::{
        wl_buffer::{self, WlBuffer},
        wl_shm::{self, Format, WlShm},
        wl_shm_pool::{self, WlShmPool},
    },
    Connection, Proxy, WEnum,
};

use crate::cursor::CursorImage;

#[derive(Debug)]
pub struct CustomCursorInternal {
    _file: File,
    shm_pool: WlShmPool,
    pub buffer: WlBuffer,
    pub w: i32,
    pub h: i32,
    pub hot_x: i32,
    pub hot_y: i32,
}

impl CustomCursorInternal {
    pub fn new(connection: &Connection, shm: &WlShm, image: &CursorImage) -> Self {
        let mfd = memfd::MemfdOptions::default()
            .close_on_exec(true)
            .create("winit-custom-cursor")
            .unwrap();
        let mut file = mfd.into_file();
        file.set_len(image.rgba.len() as u64).unwrap();
        for chunk in image.rgba.chunks_exact(4) {
            file.write_all(&[chunk[2], chunk[1], chunk[0], chunk[3]])
                .unwrap();
        }
        file.flush().unwrap();

        let pool_id = connection
            .send_request(
                shm,
                wl_shm::Request::CreatePool {
                    size: image.rgba.len() as i32,
                    fd: file.as_fd(),
                },
                Some(Arc::new(IgnoreObjectData)),
            )
            .unwrap();
        let shm_pool = WlShmPool::from_id(connection, pool_id).unwrap();

        let buffer_id = connection
            .send_request(
                &shm_pool,
                wl_shm_pool::Request::CreateBuffer {
                    offset: 0,
                    width: image.width as i32,
                    height: image.width as i32,
                    stride: (image.width as i32 * 4),
                    format: WEnum::Value(Format::Argb8888),
                },
                Some(Arc::new(IgnoreObjectData)),
            )
            .unwrap();
        let buffer = WlBuffer::from_id(connection, buffer_id).unwrap();

        CustomCursorInternal {
            _file: file,
            shm_pool,
            buffer,
            w: image.width as i32,
            h: image.height as i32,
            hot_x: image.hotspot_x as i32,
            hot_y: image.hotspot_y as i32,
        }
    }

    pub fn destroy(&self, connection: &Connection) {
        connection
            .send_request(&self.buffer, wl_buffer::Request::Destroy, None)
            .unwrap();
        connection
            .send_request(&self.shm_pool, wl_shm_pool::Request::Destroy, None)
            .unwrap();
    }
}

#[derive(Debug)]
pub enum SelectedCursor {
    Named(CursorIcon),
    Custom(CustomCursorInternal),
}

impl Default for SelectedCursor {
    fn default() -> Self {
        Self::Named(Default::default())
    }
}

struct IgnoreObjectData;

impl ObjectData for IgnoreObjectData {
    fn event(
        self: Arc<Self>,
        _: &wayland_client::backend::Backend,
        _: wayland_client::backend::protocol::Message<wayland_client::backend::ObjectId, OwnedFd>,
    ) -> Option<Arc<dyn ObjectData>> {
        None
    }
    fn destroyed(&self, _: wayland_client::backend::ObjectId) {}
}
