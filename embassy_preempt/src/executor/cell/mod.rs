//! Synchronization and interior mutability primitives

mod up;
#[allow(missing_docs)]
mod util;

pub use up::UPSafeCell;
pub use util::{SyncUnsafeCell, UninitCell};
