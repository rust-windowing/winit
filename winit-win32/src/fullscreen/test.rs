use super::{DisplayTransition, transition};

#[test]
fn preserves_modes_and_operation_order_on_success_and_failure() {
    use DisplayTransition::{Applied, Unchanged, Windowed};
    let cases = [
        (None, None, false, vec![], Applied, vec![]),
        (None, Some(2), false, vec![true], Applied, vec![(2, true)]),
        (None, Some(2), false, vec![false], Unchanged, vec![(2, true)]),
        (Some(1), None, false, vec![true], Applied, vec![(1, false)]),
        (Some(1), None, false, vec![false], Unchanged, vec![(1, false)]),
        (Some(1), Some(2), false, vec![true], Applied, vec![(2, true)]),
        (Some(1), Some(2), false, vec![false], Unchanged, vec![(2, true)]),
        (Some(1), Some(2), true, vec![false], Unchanged, vec![(1, false)]),
        (Some(1), Some(2), true, vec![true, true], Applied, vec![(1, false), (2, true)]),
        (Some(1), Some(2), true, vec![true, false, true], Unchanged, vec![
            (1, false),
            (2, true),
            (1, true),
        ]),
        (Some(1), Some(2), true, vec![true, false, false], Windowed, vec![
            (1, false),
            (2, true),
            (1, true),
        ]),
    ];
    for (old, new, changing_monitor, results, expected, expected_calls) in cases {
        let mut results = results.into_iter();
        let mut calls = Vec::new();
        let outcome = transition(old.as_ref(), new.as_ref(), changing_monitor, |mode, activate| {
            calls.push((*mode, activate));
            results.next().expect("unexpected native operation")
        });
        assert_eq!(outcome, expected, "{expected_calls:?}");
        assert_eq!(calls, expected_calls);
        assert_eq!(results.next(), None);
    }
}
