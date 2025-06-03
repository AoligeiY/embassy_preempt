//! the allocator of the heap

/// Linked_List_Allocator for [no_std]
pub mod linked_list;
/// Block_Allocator based on Linked_List_Allocator
pub mod fixed_size_block;
/// Stack_Allocator for OS_STK
pub mod stack_allocator;

pub use stack_allocator::*;
use fixed_size_block::FixedSizeBlockAllocator;

// pub const HEAP_START: *mut u8 = 0x08000200 as *mut u8;
// pub const HEAP_SIZE: usize = 100 * 1024; // 100 KiB
pub const HEAP_START: *mut u8 = (stack_allocator::STACK_START + stack_allocator::STACK_SIZE) as *mut u8;
pub const HEAP_SIZE: usize = 10 * 1024; // 100 KiB

/// Global allocator
#[global_allocator]
static ALLOCATOR: Locked<FixedSizeBlockAllocator> = Locked::new(FixedSizeBlockAllocator::new());
#[allow(unused)]
pub fn Init_Heap() {
    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }
}

/// A wrapper around spin::Mutex to permit trait implementations.
pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<A> {
        self.inner.lock()
    }
}
