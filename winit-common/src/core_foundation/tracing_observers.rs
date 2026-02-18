use std::cell::RefCell;
use std::rc::Rc;

use objc2::MainThreadMarker;
use objc2_core_foundation::{CFIndex, CFRunLoop, CFRunLoopActivity, kCFRunLoopDefaultMode};
use tracing::span::EnteredSpan;
use tracing::{Level, error, field, span_enabled, trace_span};

use crate::core_foundation::MainRunLoopObserver;

/// Create two run loop observers that add TRACE-level [spans][tracing::span].
///
/// This is useful when debugging run loops, it makes it easier to see in which run loop activity an
/// event is triggered inside (if any).
pub fn tracing_observers(
    mtm: MainThreadMarker,
) -> Option<(MainRunLoopObserver, MainRunLoopObserver)> {
    // HINT: You can use something like the following to emit relevant events:
    //
    // ```
    // tracing_subscriber::fmt()
    //     .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
    //     .with_span_events(tracing_subscriber::fmt::format::FmtSpan::ACTIVE)
    //     .init();
    // ```

    // Observers are a bit costly, so don't create them if the tracing-level for this module is
    // configured to disable them.
    if !span_enabled!(Level::TRACE) {
        return None;
    }

    /// The state that we think the runloop is currently in.
    ///
    /// The order of activities we observe if waiting twice looks something like:
    /// - CFRunLoopActivity::Entry
    /// - CFRunLoopActivity::BeforeTimers
    /// - CFRunLoopActivity::BeforeSources
    /// - CFRunLoopActivity::BeforeWaiting
    /// - CFRunLoopActivity::AfterWaiting
    /// - CFRunLoopActivity::BeforeTimers
    /// - CFRunLoopActivity::BeforeSources
    /// - CFRunLoopActivity::BeforeWaiting
    /// - CFRunLoopActivity::AfterWaiting
    /// - CFRunLoopActivity::Exit
    ///
    /// And if not waiting, it looks something like:
    /// - CFRunLoopActivity::Entry
    /// - CFRunLoopActivity::BeforeTimers
    /// - CFRunLoopActivity::BeforeSources
    /// - CFRunLoopActivity::Exit
    #[derive(Default)]
    #[allow(unused)] // EnteredSpans are kept around
    enum RunLoopState {
        /// Currently processing `Entry`/`Exit` observers.
        #[default]
        Entered,
        /// Currently processing timers or `BeforeTimers` observers.
        Timers(EnteredSpan),
        /// Currently processing sources or `BeforeSources` observers.
        Sources(EnteredSpan),
        /// Currently waiting or processing `BeforeWaiting`/`AfterWaiting` observers.
        Waiting(EnteredSpan),
    }

    // A list of currently entered (outer) spans and their state.
    //
    // This is a list because runloops can be run recursively.
    let spans: Rc<RefCell<Vec<(EnteredSpan, RunLoopState)>>> = Rc::new(RefCell::new(Vec::new()));
    let spans_clone = Rc::clone(&spans);

    // An observer at the start of run loop activities.
    let activities = CFRunLoopActivity::Entry
        | CFRunLoopActivity::BeforeTimers
        | CFRunLoopActivity::BeforeSources
        | CFRunLoopActivity::BeforeWaiting;
    let start = MainRunLoopObserver::new(mtm, activities, true, CFIndex::MIN, move |activity| {
        match activity {
            // Add an outer span for each runloop iteration.
            CFRunLoopActivity::Entry => {
                let span = trace_span!("inside runloop", mode = field::Empty);

                // Get the mode dynamically, the observer may added to multiple different modes.
                let mode = CFRunLoop::current().unwrap().current_mode().unwrap();
                // Mode isn't interesting if it's the default mode.
                if &*mode != unsafe { kCFRunLoopDefaultMode }.unwrap() {
                    span.record("mode", field::display(mode));
                }

                let entered = span.entered();
                spans.borrow_mut().push((entered, RunLoopState::Entered));
            },

            // Add inner spans that help inspecting the state the runloop is in.
            CFRunLoopActivity::BeforeTimers => {
                if let Some((_, state)) = spans.borrow_mut().last_mut() {
                    *state = RunLoopState::Entered; // Drop any previous spans.
                    *state = RunLoopState::Timers(trace_span!("processing timers").entered());
                } else {
                    error!("unbalanced observer invocations");
                }
            },
            CFRunLoopActivity::BeforeSources => {
                if let Some((_, state)) = spans.borrow_mut().last_mut() {
                    *state = RunLoopState::Entered; // Drop any previous spans.
                    *state = RunLoopState::Sources(trace_span!("processing sources").entered());
                } else {
                    error!("unbalanced observer invocations");
                }
            },
            CFRunLoopActivity::BeforeWaiting => {
                if let Some((_, state)) = spans.borrow_mut().last_mut() {
                    *state = RunLoopState::Entered; // Drop any previous spans.
                    *state = RunLoopState::Waiting(trace_span!("waiting").entered());
                } else {
                    error!("unbalanced observer invocations");
                }
            },

            activity => unreachable!("unexpected activity: {activity:?}"),
        }
    });

    // An observer at the end of run loop activities.
    let activities = CFRunLoopActivity::AfterWaiting | CFRunLoopActivity::Exit;
    let end = MainRunLoopObserver::new(mtm, activities, true, CFIndex::MAX, move |activity| {
        match activity {
            CFRunLoopActivity::AfterWaiting => {
                if let Some((_, state)) = spans_clone.borrow_mut().last_mut() {
                    // Transition from the waiting state to the initial state.
                    *state = RunLoopState::Entered;
                } else {
                    error!("unbalanced observer invocations");
                }
            },

            CFRunLoopActivity::Exit => {
                if let Some((span, state)) = spans_clone.borrow_mut().pop() {
                    drop(state); // Explicitly exit and drop inner span.
                    drop(span); // Explicitly exit and drop outer span.
                } else {
                    error!("unbalanced observer invocations");
                }
            },

            activity => unreachable!("unexpected activity: {activity:?}"),
        }
    });

    Some((start, end))
}
