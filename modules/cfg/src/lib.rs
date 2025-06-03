#![no_std]
#![no_main]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

/// the mod which define the data structure of uC/OS-II kernel
pub mod ucosii; 
/// timer timebase tick
mod tick;

use ucosii::OS_PRIO;
use cell::UPSafeCell;
// TODO: Make all the config to be feature!!!

/// the const val define the lowest prio
pub const OS_LOWEST_PRIO: OS_PRIO = 63;
/// Size of task variables array (#of INT32U entries)
pub const OS_TASK_REG_TBL_SIZE: USIZE = 1;
/// Max. number of memory partitions
pub const OS_MAX_MEM_PART: USIZE = 5;
/// Max. number of tasks in your application, MUST be >= 2
// pub const OS_MAX_TASKS: USIZE = 20;
// Max. number of event control blocks in your application
pub const OS_MAX_EVENTS: USIZE = 20;
/// This const val is used to config the size of ARENA.
/// You can set it refer to the number of tasks in your application(OS_MAX_TASKS) and the number of system tasks(OS_N_SYS_TASKS).
pub const OS_ARENA_SIZE: USIZE = 10240;
/// Ticks per second of the global timebase. Output frequency of the Timer. Frequency of the Systick(run on Timer)
/// the default one tick is 10us
/// 
///
/// This value is specified by the Cargo features "`tick-hz-*`"
pub const TICK_HZ: INT64U = tick::TICK_HZ;

lazy_static::lazy_static! {
    /// input frequency of the Timer, you should config it yourself(set the Hardware)
    pub static ref APB_HZ: UPSafeCell<INT64U> = unsafe {
        UPSafeCell::new(0)
    };
    /// the system clock frequency, you should config it yourself(set the Hardware)
    pub static ref SYSCLK_HZ: UPSafeCell<INT64U> = unsafe {
        UPSafeCell::new(0)
    };
}

/// the block delay of idle task in poll
#[cfg(feature = "delay_idle")]
pub const block_delay_poll: usize = 2;

/*
**************************************************************************************************************************************
*                                                               type define
*                                           this part needs to change according to the platform
**************************************************************************************************************************************
*/
/// ENABLE
pub const ENABLE:bool = true;
/// DISABLE
pub const DISENABLE:bool = false;
/// Unsigned  8 bit quantity
pub type BOOLEAN = bool;
/// Unsigned  8 bit quantity  
pub type INT8U = u8;
/// Signed    8 bit quantity
pub type INT8S = i8;
/// Unsigned 16 bit quantity
pub type INT16U = u16;
/// Signed   16 bit quantity
pub type INT16S = i16;
/// Unsigned 32 bit quantity
pub type INT32U = u32;
/// Signed   32 bit quantity
pub type INT32S = i32;
/// Single precision floating point
pub type FP32 = f32;
/// Double precision floating point
pub type FP64 = f64;
/// the ptr size. define this to use raw ptr
pub type PTR = *mut ();
/// the usize type used in array
pub type USIZE = usize;
/// the u64 type
pub type INT64U = u64;
/// Each stack entry is 32-bit wide
pub type OS_STK = usize;
/// Define size of CPU status register (PSR = 32 bits)
pub type OS_CPU_SR = u32;
