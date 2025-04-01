/*
********************************************************************************************************************************************
*                                                               type define
********************************************************************************************************************************************
*/

use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr::NonNull;

// #[cfg(feature = "defmt")]
#[cfg(any(feature = "alarm_test", feature = "defmt"))]
#[allow(unused_imports)]
use defmt::{info,trace};

use super::fixed_size_block::FixedSizeBlockAllocator;
use super::Locked;

pub const STACK_START: usize = 0x20000000;
pub const STACK_SIZE: usize = 20 * 1024; // 20 KiB
pub const PROGRAM_STACK_SIZE: usize = 2048; // 1KiB 512 B also ok
pub const INTERRUPT_STACK_SIZE: usize = 2048; // 1 KiB
pub const TASK_STACK_SIZE: usize = PROGRAM_STACK_SIZE; // currently we set it to the same as the program stack

use crate::port::OS_STK;
use crate::sync::UPSafeCell;
static STACK_ALLOCATOR: Locked<FixedSizeBlockAllocator> = Locked::new(FixedSizeBlockAllocator::new());
lazy_static::lazy_static! {
    pub static ref PROGRAM_STACK: UPSafeCell<OS_STK_REF> = unsafe {
        UPSafeCell::new(OS_STK_REF::default())
    };
    pub static ref INTERRUPT_STACK: UPSafeCell<OS_STK_REF> = unsafe {
        UPSafeCell::new(OS_STK_REF::default())
    };
}

/*
********************************************************************************************************************************************
*                                                           interface
********************************************************************************************************************************************
*/
/// init the stack allocator and set up the program stack and the interrupt stack
pub fn OS_InitStackAllocator() {
    #[cfg(feature = "defmt")]
    trace!("Init Stack Allocator");
    unsafe {
        STACK_ALLOCATOR.lock().init(STACK_START as *mut u8, STACK_SIZE);
    }
    // then we init the program stack
    let layout = Layout::from_size_align(PROGRAM_STACK_SIZE, 4).unwrap();
    let stk = alloc_stack(layout);
    let stk_ptr = stk.STK_REF.as_ptr() as *mut u8;
    PROGRAM_STACK.set(stk);
    // then we change the sp to the top of the program stack
    // this depending on the arch so we need extern and implement in the port
    extern "Rust" {
        fn set_program_sp(sp: *mut u8);
    }
    unsafe {
        set_program_sp(stk_ptr);
    }
    // we also need to allocate a stack for interrupt
    let layout = Layout::from_size_align(INTERRUPT_STACK_SIZE, 4).unwrap();
    let stk = alloc_stack(layout);
    INTERRUPT_STACK.set(stk);
}
/// alloc a new stack
pub fn alloc_stack(layout: Layout) -> OS_STK_REF {
    #[cfg(feature = "defmt")]
    trace!("alloc_stack");
    let heap_ptr: *mut u8;
    unsafe {
        heap_ptr = STACK_ALLOCATOR.alloc(layout);
    }
    // #[cfg(feature = "defmt")]
    #[cfg(feature = "alarm_test")]
    trace!("alloc a stack at {}", heap_ptr);
    stk_from_ptr(heap_ptr, layout)
}
/// dealloc a stack
pub fn dealloc_stack(stk: &mut OS_STK_REF) {
    #[cfg(feature = "defmt")]
    trace!("dealloc_stack");
    if stk.STK_REF == NonNull::dangling() || stk.HEAP_REF == NonNull::dangling() {
        return;
    }
    let stk_ptr = stk.HEAP_REF.as_ptr();
    stk.STK_REF = NonNull::dangling();
    stk.HEAP_REF = NonNull::dangling();
    unsafe {
        STACK_ALLOCATOR.dealloc(stk_ptr, stk.layout);
    }
}

/// the ref of the stk
pub struct OS_STK_REF {
    /// the ref of the stk(top or bottom),because the read of this
    /// field is in the asm code, so we use NonNull to ensure the safety
    /// and use #[allow(dead_code)]
    #[allow(dead_code)]
    pub STK_REF: NonNull<OS_STK>,
    /// the ref of this dynamic stk's src heap
    pub HEAP_REF: NonNull<u8>,
    /// the layout(size) of the stk
    pub layout: Layout,
}
unsafe impl Send for OS_STK_REF {}

impl Clone for OS_STK_REF {
    fn clone(&self) -> Self {
        OS_STK_REF {
            STK_REF: self.STK_REF,
            HEAP_REF: self.HEAP_REF,
            layout: self.layout,
        }
    }
}

/// when the OS_STK_REF is default, we will not alloc a stack
impl Default for OS_STK_REF {
    fn default() -> Self {
        OS_STK_REF {
            STK_REF: NonNull::dangling(),
            HEAP_REF: NonNull::dangling(),
            layout: Layout::from_size_align(0, 1).unwrap(),
        }
    }
}
/// we impl drop for OS_STK_REF to dealloc the stack(try to be RAII)
impl Drop for OS_STK_REF {
    fn drop(&mut self) {
        if self.STK_REF == NonNull::dangling() || self.HEAP_REF == NonNull::dangling() {
            return;
        }
        let stk_ptr = self.HEAP_REF.as_ptr();
        self.STK_REF = NonNull::dangling();
        self.HEAP_REF = NonNull::dangling();
        unsafe {
            STACK_ALLOCATOR.dealloc(stk_ptr, self.layout);
        }
    }
}

impl OS_STK_REF {
    pub fn as_ptr(&self) -> *mut u8 {
        self.HEAP_REF.as_ptr()
    }
}

pub fn stk_from_ptr(heap_ptr: *mut u8, layout: Layout) -> OS_STK_REF {
    OS_STK_REF {
        STK_REF: NonNull::new(unsafe { heap_ptr.offset(layout.size() as isize) as *mut OS_STK }).unwrap(),
        HEAP_REF: NonNull::new(heap_ptr).unwrap(),
        layout,
    }
}

