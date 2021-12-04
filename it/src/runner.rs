use crate::backend::{non_requirement_flags, Backend, BackendFlags};
use crate::test::TestData;
use crate::tests::Test;
use crate::tlog::LogState;
use isnt::std_1::vec::IsntVecExt;
use parking_lot::Mutex;
use rayon::prelude::*;
use std::cell::{Cell, RefCell};
use std::fs::OpenOptions;
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use tokio::task::LocalSet;

pub struct Execution {
    pub dir: PathBuf,
}

struct BackendExecution {
    dir: PathBuf,
    result: Mutex<BackendResult>,
}

#[derive(Default)]
struct BackendResult {
    failed: Vec<String>,
    not_run: Vec<(String, BackendFlags)>,
    manual_verification: Vec<String>,
}

pub fn run_tests(exec: &Execution, backend: &dyn Backend, tests: &[Box<dyn Test>]) -> bool {
    let be = BackendExecution {
        dir: exec.dir.join(backend.name()),
        result: Default::default(),
    };
    log::info!("Running tests for backend {}", backend.name());
    let rto = |test: &Box<dyn Test>| run_test_outer(&be, backend, &**test);
    if backend.flags().contains(BackendFlags::MT_SAFE) {
        tests
            .par_iter()
            .filter(|t| !t.flags().contains(BackendFlags::SINGLE_THREADED))
            .for_each(rto);
        tests
            .iter()
            .filter(|t| t.flags().contains(BackendFlags::SINGLE_THREADED))
            .for_each(rto);
    } else {
        tests.iter().for_each(rto);
    }
    let results = be.result.lock();
    if results.not_run.is_not_empty() {
        log::warn!("The following tests were not run due to missing flags:");
        for (test, flags) in &results.not_run {
            log::warn!("  - {}. Missing flags: {:?}", test, flags);
        }
    }
    if results.manual_verification.is_not_empty() {
        log::warn!("The following tests require manual verification:");
        for test in &results.manual_verification {
            log::warn!("  - {}", test);
        }
    }
    if results.failed.is_not_empty() {
        log::error!("The following tests failed:");
        for test in &results.failed {
            log::error!("  - {}", test);
        }
    }
    results.failed.is_not_empty()
}

fn run_test_outer(be: &BackendExecution, backend: &dyn Backend, test: &dyn Test) {
    let failed = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let missing_flags = test.flags() & !backend.flags() & !non_requirement_flags();
        if !missing_flags.is_empty() {
            be.result
                .lock()
                .not_run
                .push((test.name().to_string(), missing_flags));
            return false;
        }
        log::info!("Running test {}", test.name());
        run_test(&be, backend, test)
    }));
    if failed.unwrap_or(true) {
        be.result.lock().failed.push(test.name().to_string());
    } else if test.flags().contains(BackendFlags::MANUAL_VERIFICATION) {
        be.result
            .lock()
            .manual_verification
            .push(test.name().to_string());
    }
}

fn run_test(exec: &BackendExecution, backend: &dyn Backend, test: &dyn Test) -> bool {
    let test_dir = exec.dir.join(test.name());
    std::fs::create_dir_all(&test_dir).unwrap();
    let td = TestData {
        log_state: Mutex::new(LogState::new(
            OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(test_dir.join("log"))
                .unwrap(),
        )),
        test_dir,
        next_image_id: Default::default(),
        error: Cell::new(false),
        instance: RefCell::new(None),
    };
    crate::test::set_test_data_and_run(&td, || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .on_thread_park(|| {
                crate::test::with_test_data(|td| {
                    td.instance.borrow().as_ref().unwrap().before_poll();
                })
            })
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let ls = LocalSet::new();
            ls.run_until(async {
                let instance = Rc::new(backend.instantiate());
                *td.instance.borrow_mut() = Some(instance.clone());
                if tokio::time::timeout(Duration::from_secs(5), test.run(&**instance))
                    .await
                    .is_err()
                {
                    log::error!("Test timed out");
                }
                *td.instance.borrow_mut() = None;
            })
            .await;
            ls.await;
        });
        if td.error.get() {
            log::error!("Test failed due to previous error");
        }
    });
    td.error.get()
}
