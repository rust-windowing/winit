//! User-facing reporter.

use crate::{TestEvent, TestEventType, TestHandler, TestResult};
use owo_colors::OwoColorize;

use std::collections::BTreeMap;
use std::io::{self, prelude::*};

const TABSIZE: usize = 2;

/// User-facing reporter.
///
/// This reporter dumps events to the console in a user-readable format.
pub struct UserHandler {
    /// Current indent.
    indent: usize,

    /// The test set we're currently displaying.
    current_start: Option<String>,

    /// Test name we are running, if any.
    test_name: Option<String>,

    /// Cached events.
    cache: BTreeMap<String, Vec<TestEventType>>,

    /// Failures we had.
    failures: Vec<(String, String)>,
}

impl UserHandler {
    /// Create a new handler.
    pub fn new() -> Self {
        Self {
            indent: 0,
            current_start: None,
            test_name: None,
            cache: BTreeMap::new(),
            failures: vec![],
        }
    }

    /// Process the provided events.
    fn process_events(&mut self, events: impl IntoIterator<Item = TestEvent>) {
        for event in events {
            // Tell if this is an end event.
            let mut ender = matches!(event.ty, TestEventType::Complete { .. });

            // If there is no test name set, run the current one.
            match self.current_start.as_ref() {
                None => {
                    let TestEvent { runner, ty } = event;
                    self.current_start = Some(runner);
                    self.dump_events(Some(ty));
                }

                Some(test_name) => {
                    // If there is a test name set and it's ours, post it immediately.
                    if test_name == &event.runner {
                        self.dump_events(Some(event.ty));
                    } else {
                        // Add it to the back of another one of the events.
                        self.cache
                            .entry(test_name.clone())
                            .or_default()
                            .push(event.ty);
                    }
                }
            }

            // If this is the end, dump other events.
            while ender {
                assert!(self.current_start.take().is_some());

                // Pick one set.
                if let Some(entry) = self.cache.first_entry() {
                    let (test_name, entries) = entry.remove_entry();
                    self.current_start = Some(test_name);

                    // Dump events and look for a conclusion.
                    ender = false;
                    self.dump_events(entries.into_iter().inspect(|ty| {
                        ender |= matches!(ty, TestEventType::Complete { .. });
                    }));
                }
            }
        }
    }

    /// Dump the provided events to the console.
    fn dump_events(&mut self, events: impl IntoIterator<Item = TestEventType>) {
        let mut stdout = io::stdout().lock();

        for event in events {
            // Write the indent.
            for _ in 0..(self.indent * TABSIZE) {
                stdout.write_all(b" ").unwrap();
            }

            match event {
                TestEventType::GroupStarted { name } => {
                    assert!(self.test_name.is_none());

                    // Write the group name and bump the indent.
                    writeln!(stdout, "{}", name.yellow().italic()).unwrap();

                    // Add to the indent.
                    self.indent += 1;
                }

                TestEventType::GroupEnded => {
                    assert!(self.test_name.is_none());

                    // Drop the indent.
                    self.indent = self.indent.checked_sub(1).unwrap();
                }

                TestEventType::TestStarted { name } => {
                    assert!(self.test_name.is_none());

                    // Write the line.
                    write!(stdout, "{} ", name.white().italic()).unwrap();
                    self.test_name = Some(name);
                }

                TestEventType::TestEnded { result } => {
                    let test_name = self.test_name.take().unwrap();

                    // Write the result.
                    match result {
                        TestResult::Passed => {
                            writeln!(stdout, "{}", "ok".green().bold()).unwrap();
                        }

                        TestResult::Failed(failure) => {
                            self.failures.push((test_name, failure));
                            writeln!(stdout, "{}", "FAIL".red().bold()).unwrap();
                        }

                        TestResult::Skipped => {
                            writeln!(stdout, "{}", "skipped".yellow().bold()).unwrap();
                        }
                    }
                }

                _ => {
                    // Completion.
                }
            }
        }
    }
}

impl TestHandler for UserHandler {
    fn handle_test(&mut self, event: TestEvent) {
        self.process_events(Some(event));
    }
}

impl Drop for UserHandler {
    fn drop(&mut self) {
        assert!(self.cache.is_empty());
    }
}
