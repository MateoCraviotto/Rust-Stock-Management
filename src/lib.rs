use std::sync::atomic::AtomicU8;

use common::log::Verbosity;

pub mod common;
pub mod ecommerce;
pub mod local;

pub static VERBOSITY: AtomicU8 = AtomicU8::new(Verbosity::Debug as u8);
