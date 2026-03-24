use sctk::shell::xdg::popup::PopupHandler;

#[derive(Default, Debug)]
pub struct State {}

impl PopupHandler for State {
    fn configure(
        &mut self,
        conn: &wayland_client::Connection,
        qh: &wayland_client::QueueHandle<Self>,
        popup: &sctk::shell::xdg::popup::Popup,
        config: sctk::shell::xdg::popup::PopupConfigure,
    ) {
    }

    fn done(
        &mut self,
        conn: &wayland_client::Connection,
        qh: &wayland_client::QueueHandle<Self>,
        popup: &sctk::shell::xdg::popup::Popup,
    ) {
    }
}
