#![no_std]

mod lock;

#[cfg(not(feature = "log-selected"))]
compile_error!("Exactly one of the `log` or `defmt` features must be enabled");

#[cfg(all(feature = "log", feature = "defmt"))]
compile_error!("At most one of the `log` and `defmt` features may be anbled.");

#[cfg(feature = "log-selected")]
pub mod log;

#[cfg(feature = "log-selected")]
pub mod spsc;
