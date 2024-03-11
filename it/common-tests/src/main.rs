//! Run the test.

use gui_test::{test, Harness};
use macro_rules_attribute::apply;

use winit::event_loop::EventLoop;

#[allow(deprecated)]
#[apply(test)]
fn initialize(harness: &mut Harness) {
    let mut group = harness.group("sanity");
    group.harness().with_test("startup/shutdown", || {
        let evl = EventLoop::new().expect("initialization");
        evl.run(|_event, elwt| {
            elwt.exit();
        })
        .expect("running");
    });
}

gui_test::main! {
    gui_test::remote::handler()
}
