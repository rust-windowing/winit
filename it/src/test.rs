use crate::backend::Instance;
use crate::tlog::LogState;
use parking_lot::Mutex;
use std::cell::{Cell, RefCell};
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::ptr;
use std::rc::Rc;
use std::sync::atomic::AtomicUsize;

pub struct TestData {
    pub test_dir: PathBuf,
    pub next_image_id: AtomicUsize,
    pub log_state: Mutex<LogState>,
    pub error: Cell<bool>,
    pub instance: RefCell<Option<Rc<Box<dyn Instance>>>>,
}

thread_local! {
    static TEST: Cell<*const TestData> = Cell::new(ptr::null());
}

pub fn set_test_data_and_run<T, F: FnOnce() -> T>(td: &TestData, f: F) -> T {
    TEST.with(|t| {
        assert!(t.get().is_null());
        t.set(td);
        let res = std::panic::catch_unwind(AssertUnwindSafe(f));
        t.set(ptr::null());
        match res {
            Ok(t) => t,
            Err(e) => std::panic::resume_unwind(e),
        }
    })
}

pub fn with_test_data<T, F: FnOnce(&TestData) -> T>(f: F) -> T {
    TEST.with(|t| {
        assert!(!t.get().is_null());
        unsafe { f(&*t.get()) }
    })
}

pub fn has_test_data() -> bool {
    TEST.with(|t| !t.get().is_null())
}
