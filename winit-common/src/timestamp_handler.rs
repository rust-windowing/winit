use core::time::Duration;

#[derive(Debug, Clone, Copy, Default)]
pub struct TimeStampExtender {
    pub last: Option<u64>,
}

impl TimeStampExtender {
    /// Convert a timestamp from u32 to duration by preventing also the wrap
    pub fn extend_timestamp(&mut self, timestamp_millis: u32) -> Duration {
        let Some(last) = self.last else {
            self.last = Some(timestamp_millis as u64);
            return Duration::from_millis(timestamp_millis as u64);
        };

        let delta = timestamp_millis.wrapping_sub(last as u32) as i32 as i64;
        // converting last to i64 is fine here because 2^63 - 1 milliseconds is still enough
        let extended = (last as i64 + delta).max(0) as u64;
        if delta > 0 {
            self.last = Some(extended);
        }
        Duration::from_millis(extended)
    }
}

#[cfg(test)]
mod tests {
    use core::time::Duration;

    use crate::timestamp_handler::TimeStampExtender;

    #[test]
    fn test_extend_timestamp() {
        const TEST_CASES: &[(u32, u32, Duration)] = &[
            (10, 20, Duration::from_millis(20)),
            (u32::MAX - 100, u32::MAX, Duration::from_millis(u32::MAX as u64)),
            (u32::MAX - 100, 0, Duration::from_millis(u32::MAX as u64 + 1)),
            (u32::MAX - 100, 200, Duration::from_millis(u32::MAX as u64 + 200 + 1)),
            (u32::MAX, u32::MAX - 23, Duration::from_millis(u32::MAX as u64 - 23)),
            (10, 3, Duration::from_millis(3)),
            // it is more realistic that the second timestamp is before the current
            // one than around 49days later
            (10, u32::MAX - 5, Duration::ZERO),
        ];

        for (index, (start_value, new_timestamp, expected)) in TEST_CASES.iter().enumerate() {
            let mut timestamp_extender = TimeStampExtender::default();
            timestamp_extender.extend_timestamp(*start_value);

            assert_eq!(
                timestamp_extender.extend_timestamp(*new_timestamp).as_nanos(),
                expected.as_nanos(),
                "Failed index: {index}"
            );
        }
    }
}
