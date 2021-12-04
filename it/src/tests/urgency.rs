use crate::backend::{BackendFlags, Instance};
use winit::window::UserAttentionType;

test!(run, BackendFlags::WINIT_SET_ATTENTION);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();

    {
        let window = el.create_window(Default::default());
        window.attention(false).await;
        window.winit_set_attention(Some(UserAttentionType::Critical));
        window.attention(true).await;
        window.winit_set_attention(None);
        window.attention(false).await;
    }
}
