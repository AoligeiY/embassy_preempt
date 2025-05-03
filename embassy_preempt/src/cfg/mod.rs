use crate::port::*;
use crate::executor::cell::UPSafeCell;
use ucosii::OS_PRIO;

/// the mod which define the data structure of uC/OS-II kernel
pub mod ucosii; 

/// timer timebase tick
mod tick;

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
