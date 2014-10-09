#![feature(phase)]
#![feature(tuple_indexing)]

extern crate glutin;

#[cfg(feature = "headless")]
#[test]
fn main() {
    let window = glutin::HeadlessRendererBuilder::new(1024, 768).build().unwrap();

    unsafe { window.make_current() };

}
