/// Allows for using `log` macros when the feature is disabled.
#[allow(unused)]
#[cfg(not(feature = "log"))]
#[macro_use]
mod log {
    macro_rules! debug {
        ($($t:tt)*) => {}
    }

    macro_rules! error {
        ($($t:tt)*) => {}
    }

    macro_rules! info {
        ($($t:tt)*) => {}
    }

    macro_rules! log {
        ($($t:tt)*) => {}
    }

    macro_rules! log_enabled {
        ($($t:tt)*) => { false }
    }

    macro_rules! trace {
        ($($t:tt)*) => {}
    }

    macro_rules! warn {
        ($($t:tt)*) => {}
    }
}
