#![no_std]
#![no_main]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

//! Synchronization and interior mutability primitives
mod up;
#[allow(missing_docs)]
mod util;

pub use up::UPSafeCell;
pub use util::{SyncUnsafeCell, UninitCell};
