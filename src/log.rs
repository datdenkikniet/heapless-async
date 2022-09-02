pub use self::log::*;

#[cfg(feature = "defmt")]
mod log {
    pub use defmt::{debug, error, info, trace, warn};
}

#[cfg(feature = "std")]
mod log {
    pub use log::{debug, error, info, trace, warn};
}
