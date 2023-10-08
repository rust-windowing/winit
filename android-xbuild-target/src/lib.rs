use winit::platform::android::{
    activity::AndroidApp,
    EventLoopBuilderExtAndroid
};

#[no_mangle]
fn android_main(app: AndroidApp) {
    winit::event_loop::EventLoopBuilder::new()
        .with_android_app(app)
        .build()
        .unwrap()
        .run(|_, _| todo!())
        .ok();
}
