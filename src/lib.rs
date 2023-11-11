use std::sync::atomic::AtomicU8;

use common::log::Verbosity;

pub mod local;
pub mod ecommerce;
pub mod common;

pub static VERBOSITY: AtomicU8 = AtomicU8::new(Verbosity::Debug as u8);
