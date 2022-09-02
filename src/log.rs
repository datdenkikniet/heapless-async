pub use self::log::*;

#[cfg(feature = "log-defmt")]
mod log {
    pub use defmt::{debug, error, info, trace, warn};
}

#[cfg(feature = "log-log")]
mod log {
    pub use log::{debug, error, info, trace, warn};
}

#[cfg(not(any(feature = "log-defmt", feature = "log-log")))]
#[allow(missing_docs)]
mod log {

    #[macro_export]
    macro_rules! debug {
        ($in:tt) => {};
        ($($in:tt),*) => {};
    }

    #[macro_export]
    macro_rules! trace {
        ($in:tt) => {};
        ($($in:tt),*) => {};
    }

    #[macro_export]
    macro_rules! info {
        ($in:tt) => {};
        ($($in:tt),*) => {};
    }

    pub use {debug, info, trace};
}
