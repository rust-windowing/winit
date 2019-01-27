#[cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd"))]
extern crate smithay_client_toolkit as sctk;

#[cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd"))]
use winit::os::unix::WindowExt;

// Wayland requires the commiting of a surface to display a window
pub fn init_wayland(window: &winit::Window) {
    #[cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd"))]
    {
        if let Some(winit_display) = window.get_wayland_display() {
            if let Some(surface) = window.get_wayland_surface() {
                use self::sctk::reexports::client::protocol::wl_shm;
                use self::sctk::reexports::client::protocol::wl_surface::RequestsTrait as SurfaceRequests;
                use self::sctk::reexports::client::{Display, Proxy};
                use self::sctk::utils::DoubleMemPool;
                use self::sctk::wayland_client::sys::client::wl_display;
                use self::sctk::Environment;

                let (width, height): (u32, u32) = window.get_inner_size().unwrap().into();
                let (display, mut event_queue) =
                    unsafe { Display::from_external_display(winit_display as *mut wl_display) };
                let env = Environment::from_display(&*display, &mut event_queue).unwrap();
                let mut pools =
                    DoubleMemPool::new(&env.shm, || {}).expect("Failed to create a memory pool !");
                let surface = unsafe { Proxy::from_c_ptr(surface as *mut _) };

                if let Some(pool) = pools.pool() {
                    pool.resize(4 * (width * height) as usize)
                        .expect("Failed to resize the memory pool.");
                    let new_buffer = pool.buffer(
                        0,
                        width as i32,
                        height as i32,
                        4 * width as i32,
                        wl_shm::Format::Argb8888,
                    );
                    surface.attach(Some(&new_buffer), 0, 0);
                    surface.commit();
                    event_queue.sync_roundtrip().unwrap();
                }
            }
        }
    }
}
