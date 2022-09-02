#![doc = include_str!("../README.md")]
#![no_std]
#![deny(missing_docs)]

mod lock;
mod waker;

pub(crate) mod log;

pub mod spsc;
