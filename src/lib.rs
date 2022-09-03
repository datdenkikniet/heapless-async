#![doc = include_str!("../README.md")]
#![no_std]
#![deny(missing_docs)]

mod mutex;
mod waker;

pub(crate) mod log;

pub mod mpmc;
pub mod spsc;
