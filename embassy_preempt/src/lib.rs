#![no_std]
#![no_main]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![feature(allocator_api)]
#![feature(alloc_layout_extra)]
#![feature(slice_ptr_get)]
#![feature(sync_unsafe_cell)]
#![feature(alloc_error_handler)]
#![feature(const_mut_refs)]
#![feature(never_type)]
#![warn(missing_docs)]
#![feature(naked_functions)]
// by noah：generate the ucosii static lib
#![crate_type = "staticlib"]
//! the mod of uC/OS-II kernel and the interface that uC/OS-II kernel provides
/// This mod MUST go first, so that the others see its macros.
/*
********************************************************************************************************************************************
*                                                               pub mod
********************************************************************************************************************************************
*/
#[cfg(any(feature = "defmt",feature = "alarm_test"))]
use defmt_rtt as _;
// global logger
pub extern crate alloc;

/// the apps
pub mod app;
/// need to import port here
pub mod cfg;
/// the async scheduler(executor) of rust uC
pub mod executor;
/// the mod of time of uC/OS-II kernel
pub mod os_time;
/// need to import port here
pub mod port;
/// the mod of event of uC/OS-II kernel
pub mod event;

/// the the macro of atomic operation
#[macro_use]
/// the atomic_macros module is used to define atomic operations
pub mod atomic_macros;

// This must go last, so that it sees all the impl_foo! macros defined earlier.
// pub(crate) mod _generated {
//     #![allow(dead_code)]
//     #![allow(unused_imports)]
//     #![allow(non_snake_case)]
//     #![allow(missing_docs)]

//     include!(concat!(env!("OUT_DIR"), "/_generated.rs"));
// }

// pac
#[cfg(feature = "unstable-pac")]
pub use stm32_metapac as pac;
#[cfg(not(feature = "unstable-pac"))]
pub(crate) use stm32_metapac as pac;
