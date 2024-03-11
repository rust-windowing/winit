//! A testing framework that can be run remotely.

pub mod remote;
pub mod stream;
pub mod user;

use serde::{Deserialize, Serialize};

use std::env;
use std::ffi::{OsStr, OsString};
use std::mem;
use std::num::NonZeroUsize;
use std::panic;

const GUI_TEST_CURRENT_TEST_NAME: &str = "GUI_TEST_CURRENT_TEST_NAME";
const GUI_TEST_SUBPROCESS_LIMIT: &str = "GUI_TEST_SUBPROCESS_LIMIT";
const DEFAULT_LIMIT: usize = 1;

#[doc(hidden)]
pub use inventory as __inventory;

/// Replacement for the `main` function.
#[macro_export]
macro_rules! main {
    ($handler:expr) => {
        fn main() {
            $crate::__entry(|| $handler)
        }
    };
}

/// Set up a test for the test framework.
#[macro_export]
macro_rules! test {
    (
        $(#[$attr:meta])*
        fn $name:ident ($hname:ident : $htype:ty) $bl:block
    ) => {
        const _: () = {
            $(#[$attr])*
            fn $name ($hname: $htype) $bl

            $crate::__inventory::submit! {
                $crate::__TestStart::__new(
                    stringify!($name),
                    $name
                )
            }
        };
    };
}

/// Test start.
#[doc(hidden)]
pub struct __TestStart {
    /// The name of the test.
    name: &'static str,

    /// The function to call.
    func: fn(&mut Harness),
}

impl __TestStart {
    /// Create a new test start.
    #[doc(hidden)]
    pub const fn __new(name: &'static str, func: fn(&mut Harness)) -> Self {
        Self { name, func }
    }
}

inventory::collect! {
    __TestStart
}

/// A harness for running the tests.
pub struct Harness {
    /// Name of the test start.
    name: String,

    /// The inner test handler.
    handler: Box<dyn TestHandler + Send + 'static>,

    /// Number of tests that have been run so far.
    test_count: usize,

    /// Number of tests that have failed so far.
    test_fails: usize,

    /// Number of tests that have succeeded.
    test_passed: usize,

    /// Current state of the test harness.
    state: State,
}

impl Harness {
    /// Create a new test harness.
    fn new<H: TestHandler + Send + 'static>(name: &str, handler: H) -> Self {
        Self {
            name: name.to_string(),
            handler: Box::new(handler),
            test_count: 0,
            test_fails: 0,
            test_passed: 0,
            state: State::Default,
        }
    }

    /// Begin a test.
    pub fn test(&mut self, name: impl Into<String>) -> Testing<'_> {
        // Make sure we aren't mid test.
        match mem::replace(&mut self.state, State::Default) {
            State::InTest { past_groups } => {
                self.state = State::InTest { past_groups };
                panic!("tried to start a test while another was underway");
            }

            State::InGroups(groups) => {
                self.state = State::InTest {
                    past_groups: Some(groups),
                };
            }

            State::Default => {
                self.state = State::InTest { past_groups: None };
            }
        }

        // Send the "test started" event to the handler.
        self.send_event(TestEventType::TestStarted { name: name.into() });

        // Return the handle.
        Testing {
            harness: Some(self),
        }
    }

    /// Run a closure as a test.
    pub fn with_test<T>(&mut self, name: impl Into<String>, f: impl FnOnce() -> T) -> T {
        let test = self.test(name.into());
        match panic::catch_unwind(panic::AssertUnwindSafe(f)) {
            Ok(x) => x,

            Err(err) => {
                if let Some(panic) = err.downcast_ref::<&'static str>() {
                    test.fail(panic.to_string());
                } else if let Some(panic) = err.downcast_ref::<String>() {
                    test.fail(panic.clone());
                } else {
                    test.fail("unintelligible error".to_string());
                }

                panic::resume_unwind(err)
            }
        }
    }

    /// Begin a test group.
    pub fn group(&mut self, name: impl Into<String>) -> Grouping<'_> {
        // Make sure we can begin a group.
        match mem::replace(&mut self.state, State::Default) {
            State::Default => {
                self.state = State::InGroups(NonZeroUsize::new(1).unwrap());
            }
            State::InGroups(groups) => {
                self.state = State::InGroups(groups.checked_add(1).unwrap());
            }
            State::InTest { past_groups } => {
                self.state = State::InTest { past_groups };
                panic!("cannot start group mid-test")
            }
        }

        // Send the "group started" event to the handler.
        self.send_event(TestEventType::GroupStarted { name: name.into() });

        // Return the handle.
        Grouping { harness: self }
    }

    /// Run a closure inside of a group.
    pub fn with_group<T>(&mut self, name: impl Into<String>, f: impl FnOnce(&mut Self) -> T) -> T {
        let mut group = self.group(name.into());
        f(group.harness())
    }

    /// End an ongoing test.
    fn end_test(&mut self, reason: TestResult) {
        self.test_count += 1;
        match &reason {
            TestResult::Passed => self.test_passed += 1,
            TestResult::Failed(..) => self.test_fails += 1,
            _ => {}
        }

        self.send_event(TestEventType::TestEnded { result: reason });

        let count = match mem::replace(&mut self.state, State::Default) {
            State::InTest { past_groups } => past_groups,
            _ => unreachable!(),
        };

        self.state = match count {
            None => State::Default,
            Some(count) => State::InGroups(count),
        };
    }

    /// End the current group.
    fn end_group(&mut self) {
        self.send_event(TestEventType::GroupEnded);

        let count = match mem::replace(&mut self.state, State::Default) {
            State::InGroups(groups) => groups,
            _ => unreachable!(),
        };

        self.state = match NonZeroUsize::new(count.get() - 1) {
            None => State::Default,
            Some(groups) => State::InGroups(groups),
        };
    }

    /// Send a test event of the provided type.
    fn send_event(&mut self, ty: TestEventType) {
        let event = TestEvent {
            runner: self.name.clone(),
            ty,
        };

        self.handler.handle_test(event);
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.send_event(TestEventType::Complete {
            total: self.test_count,
            fail: self.test_fails,
            pass: self.test_passed,
        });
    }
}

/// An in-progress test.
pub struct Testing<'a> {
    harness: Option<&'a mut Harness>,
}

impl Testing<'_> {
    /// Skip this test.
    pub fn skip(mut self) {
        // Send the "skipped" event.
        self.harness.take().unwrap().end_test(TestResult::Skipped);
    }

    /// Fail this test.
    fn fail(mut self, panic: String) {
        self.harness
            .take()
            .unwrap()
            .end_test(TestResult::Failed(panic));
    }
}

impl Drop for Testing<'_> {
    fn drop(&mut self) {
        if let Some(harness) = self.harness.take() {
            let result = if std::thread::panicking() {
                TestResult::Failed("thread panicked".into())
            } else {
                TestResult::Passed
            };

            harness.end_test(result);
        }
    }
}

/// We are running a test group.
pub struct Grouping<'a> {
    harness: &'a mut Harness,
}

impl Grouping<'_> {
    /// Get the underlying test harness.
    pub fn harness(&mut self) -> &mut Harness {
        &mut self.harness
    }
}

impl Drop for Grouping<'_> {
    fn drop(&mut self) {
        self.harness.end_group();
    }
}

/// Current testing state.
enum State {
    /// We are in the middle of this many groups.
    InGroups(NonZeroUsize),

    /// We are in the middle of a test.
    InTest { past_groups: Option<NonZeroUsize> },

    /// We are in the default state.
    Default,
}

/// A handler for incoming test events.
pub trait TestHandler {
    /// Handle a test.
    fn handle_test(&mut self, event: TestEvent);
}

impl<T: TestHandler + ?Sized> TestHandler for Box<T> {
    fn handle_test(&mut self, event: TestEvent) {
        (**self).handle_test(event)
    }
}

/// An event produced by the test harness.
#[derive(Debug, Serialize, Deserialize)]
pub struct TestEvent {
    /// The name of the runner associated with the event.
    pub runner: String,

    /// The type of the event.
    pub ty: TestEventType,
}

/// The type of the event.
#[non_exhaustive]
#[derive(Debug, Serialize, Deserialize)]
pub enum TestEventType {
    /// The tests are complete and the harness can be disconnected.
    Complete {
        /// Total number of tests.
        total: usize,

        /// Total number of passing tests.
        pass: usize,

        /// Total number of failed tests.
        fail: usize,
    },

    /// A test has started.
    TestStarted { name: String },

    /// A test has completed.
    TestEnded { result: TestResult },

    /// A test group has started.
    GroupStarted { name: String },

    /// A test group has ended.
    GroupEnded,
}

/// The result of a test.
#[non_exhaustive]
#[derive(Debug, Serialize, Deserialize)]
pub enum TestResult {
    /// The test passed.
    Passed,

    /// The test failed with the provided error.
    Failed(String),

    /// The test was skipped.
    Skipped,
}

/// Entry point of the test.
#[doc(hidden)]
pub fn __entry<H: TestHandler + Send + 'static>(handler: impl FnOnce() -> H) {
    // Look for the test name environment variable.
    if let Some(test_name) = env::var(GUI_TEST_CURRENT_TEST_NAME)
        .ok()
        .filter(|test_name| !test_name.is_empty())
    {
        // Find the provided test.
        let test_to_run = inventory::iter::<__TestStart>
            .into_iter()
            .find(|test| test.name == test_name)
            .unwrap_or_else(|| panic!("unable to find test '{test_name}'"));

        // Create a harness.
        let mut harness = Harness::new(test_to_run.name, handler());

        // Run the test.
        panic::catch_unwind(panic::AssertUnwindSafe(move || {
            (test_to_run.func)(&mut harness)
        }))
        .ok();
    } else {
        // Run a subprocess for every test.
        let limit = env::var(GUI_TEST_SUBPROCESS_LIMIT)
            .ok()
            .and_then(|limit| limit.parse::<usize>().ok())
            .unwrap_or(DEFAULT_LIMIT);
        let process_name = env::args_os().next().unwrap();

        let sema = async_lock::Semaphore::new(limit);
        let ex = async_executor::Executor::new();

        async_io::block_on(ex.run(async {
            let mut tasks = vec![];

            // Set up an environment variable for this.
            for test in inventory::iter::<__TestStart> {
                // Acquire a guard.
                let guard = sema.acquire().await;

                // Spawn a subprocess.
                let mut process = async_process::Command::new(&process_name)
                    .envs(env::vars_os().chain(Some({
                        (path(&GUI_TEST_CURRENT_TEST_NAME), path(&test.name))
                    })))
                    .spawn()
                    .expect("failed to spawn child process");

                // Spawn a task to poll that subprocess.
                let task = ex.spawn(async move {
                    let _guard = guard;
                    process.status().await.unwrap()
                });

                tasks.push(task);
            }

            // Finish all of the tasks.
            for task in tasks {
                task.await;
            }
        }));
    }
}

fn path<A: AsRef<OsStr>>(s: &A) -> OsString {
    s.as_ref().into()
}
