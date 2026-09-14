/// Deliver an already-collected scheduled batch in its selected order.
///
/// Resolve the current target immediately before each callback: an earlier
/// callback may replace a later target. State/property changes on the same
/// block type still receive their tick, but a replacement type must not.
/// Selection, tick limits and scheduling during callbacks belong to the caller.
pub(super) fn dispatch<P, T, S>(
    ticks: impl IntoIterator<Item = (P, T)>,
    mut read: impl FnMut(&P) -> S,
    mut matches: impl FnMut(&S, &T) -> bool,
    mut run: impl FnMut(S, &P),
) {
    for (position, target) in ticks {
        let current = read(&position);
        if matches(&current, &target) {
            run(current, &position);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    #[test]
    fn earlier_callback_can_invalidate_a_later_target_across_batch_boundaries() {
        let states = RefCell::new(vec![(0u8, 0u8); 65]);
        let mut seen = Vec::new();
        super::dispatch(
            (0..65).map(|position| (position, 0)),
            |position| states.borrow()[*position],
            |current, target| current.0 == *target,
            |current, position| {
                seen.push((*position, current.1));
                if *position == 0 {
                    states.borrow_mut()[32] = (1, 0);
                    states.borrow_mut()[64] = (0, 9);
                }
            },
        );
        let expected: Vec<_> = (0..65)
            .filter(|position| *position != 32)
            .map(|position| (position, if position == 64 { 9 } else { 0 }))
            .collect();
        assert_eq!(seen, expected);
    }

    #[test]
    fn dispatch_retains_collected_order_even_when_positions_run_backwards() {
        let mut seen = Vec::new();
        super::dispatch(
            (0..97).rev().map(|position| (position, 0u8)),
            |_| 0,
            |current, target| current == target,
            |_, position| seen.push(*position),
        );
        assert_eq!(seen, (0..97).rev().collect::<Vec<_>>());
    }
}
