pub use self::log::*;

#[cfg(feature = "defmt")]
mod log {
    pub(crate) use defmt::{debug, error, info, trace, warn};
}

#[cfg(feature = "log")]
mod log {
    pub(crate) use log::{debug, error, info, trace, warn};
}

#[cfg(not(any(feature = "defmt", feature = "log")))]
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
