//! Run the test.

use gui_test::{test, Harness};
use macro_rules_attribute::apply;

use winit::event_loop::EventLoop;

#[apply(test)]
fn initialize(harness: &mut Harness) {
    let _test = harness.test("startup/shutdown");

    let evl = EventLoop::new().unwrap();
    evl.run(|_event, elwt| {
        elwt.exit();
    })
    .unwrap();
}

gui_test::main! {
    gui_test::remote::handler()
}
