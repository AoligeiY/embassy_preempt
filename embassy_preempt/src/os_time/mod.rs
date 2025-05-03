#[cfg(feature = "defmt")]
#[allow(unused_imports)]
use defmt::{info, trace};
#[cfg(feature = "alarm_test")]
use defmt::{info, trace};
use core::sync::atomic::Ordering;

use crate::executor::{wake_task_no_pend, GlobalSyncExecutor};
use crate::port::time_driver::{Driver, RTC_DRIVER};
use crate::port::{INT8U, INT64U, USIZE};
use crate::cfg::{TICK_HZ, OS_LOWEST_PRIO};
use crate::cfg::ucosii::{OSRunning, OSIntNesting, OSLockNesting, OS_ERR_STATE};
/// the mod of blockdelay of uC/OS-II kernel
pub mod blockdelay;
/// the mod of duration of uC/OS-II kernel
pub mod duration;
/// the mod of instant of uC/OS-II kernel
pub mod instant;
/// the mod of timer of uC/OS-II kernel
pub mod timer;

/// init the Timer as the Systick
pub fn OSTimerInit() {
    #[cfg(feature = "defmt")]
    trace!("OSTimerInit");
    RTC_DRIVER.init();
}

/// delay async task 'n' ticks
pub(crate) unsafe fn delay_tick(_ticks: INT64U) {
    // by noah：Remove tasks from the ready queue in advance to facilitate subsequent unified operations
    let executor = GlobalSyncExecutor.as_ref().unwrap();
    let task = executor.OSTCBCur.get_mut();
    task.expires_at.set(RTC_DRIVER.now() + _ticks);
    // update timer
    let mut next_expire = critical_section::with(|_| {
        executor.set_task_unready(*task);
        critical_section::with(|_| executor.timer_queue.update(*task))
    });
    #[cfg(feature = "defmt")]
    trace!("in delay_tick the next expire is {:?}", next_expire);
    if critical_section::with(|_| {
        if next_expire < *executor.timer_queue.set_time.get_unmut() {
            executor.timer_queue.set_time.set(next_expire);
            true
        } else {
            // if the next_expire is not less than the set_time, it means the expire dose not arrive, or the task
            // dose not expire a timestamp so we should set the task unready
            false
        }
    }) {
        // by noah：if the set alarm return false, it means the expire arrived.
        // So we can not set the **task which is waiting for the next_expire** as unready
        // The **task which is waiting for the next_expire** must be current task
        // we must do this until we set the alarm successfully or there is no alarm required
        while !RTC_DRIVER.set_alarm(executor.alarm, next_expire) {
            // by noah: if set alarm failed, it means the expire arrived, so we should not set the task unready
            // we should **dequeue the task** from time_queue, **clear the set_time of the time_queue** and continue the loop
            // (just like the operation in alarm_callback)
            executor
                .timer_queue
                .dequeue_expired(RTC_DRIVER.now(), wake_task_no_pend);
            // then we need to set a new alarm according to the next expiration time
            next_expire = unsafe { executor.timer_queue.next_expiration() };
            #[cfg(feature = "defmt")]
            trace!("in delay_tick the next expire is {:?}", next_expire);
            // by noah：we also need to updater the set_time of the timer_queue
            executor.timer_queue.set_time.set(next_expire);
        }
    }
    // find the highrdy
    if critical_section::with(|_| {
        executor.set_highrdy();
        executor.OSPrioHighRdy != executor.OSPrioCur
    }) {
        // call the interrupt poll
        GlobalSyncExecutor.as_ref().unwrap().interrupt_poll();
        #[cfg(feature = "defmt")]
        trace!("end the delay");
    }
}


/// we have to make this delay acting like preemptive delay
pub fn OSTimeDly(_ticks: INT64U) {
    #[cfg(feature = "defmt")]
    trace!("OSTimeDly");
    // See if trying to call from an ISR  
    if OSIntNesting.load(Ordering::Acquire) > 0 {
        return;
    }
    // See if called with scheduler locked
    if OSLockNesting.load(Ordering::Acquire) > 0 {
        return;
    }
    unsafe {
        delay_tick(_ticks);
    }
}


/*
*********************************************************************************************************
*                                    DELAY TASK FOR SPECIFIED TIME
*
* Description: This function is called to delay execution of the currently running task until some time
*              expires.  This call allows you to specify the delay time in HOURS, MINUTES, SECONDS and
*              MILLISECONDS instead of ticks.
*
* Arguments  : hours     specifies the number of hours that the task will be delayed (max. is 255)
*              minutes   specifies the number of minutes (max. 59)
*              seconds   specifies the number of seconds (max. 59)
*              ms        specifies the number of milliseconds (max. 999)
*
* Returns    : OS_ERR_NONE
*              OS_ERR_TIME_INVALID_MINUTES
*              OS_ERR_TIME_INVALID_SECONDS
*              OS_ERR_TIME_INVALID_MS
*              OS_ERR_TIME_ZERO_DLY
*              OS_ERR_TIME_DLY_ISR
*
* Note(s)    : The resolution on the milliseconds depends on the tick rate.  For example, you can't do
*              a 10 mS delay if the ticker interrupts every 100 mS. 
*********************************************************************************************************
*/
#[cfg(feature = "OS_TIME_DLY_HMSM_EN")]
/// this call allows you to specify the delay time
pub fn OSTimeDlyHMSM(hours: INT8U, minutes: INT8U, seconds: INT8U, ms: INT64U) -> OS_ERR_STATE {
    #[cfg(feature = "defmt")]
    trace!("OSTimeDlyHMSM");
    // See if trying to call from an ISR  
    if OSIntNesting.load(Ordering::Acquire) > 0 {
        return  OS_ERR_STATE::OS_ERR_TIME_DLY_ISR;
    }
    // See if called with scheduler locked
    if OSLockNesting.load(Ordering::Acquire) > 0 {
        return  OS_ERR_STATE::OS_ERR_SCHED_LOCKED;
    }

    #[cfg(feature = "OS_ARG_CHK_EN")]
    {
        if hours == 0 {
            if minutes == 0 {
                if seconds == 0 {
                    if ms == 0 {
                        return OS_ERR_STATE::OS_ERR_TIME_ZERO_DLY;
                    }
                }
            }
        }
        // Validate arguments to be within range 
        if minutes > 59 {
            return OS_ERR_STATE::OS_ERR_TIME_INVALID_MINUTES;
        }
        if seconds > 59 {
            return OS_ERR_STATE::OS_ERR_TIME_INVALID_SECONDS;
        }
        if ms > 999 {
            return OS_ERR_STATE::OS_ERR_TIME_INVALID_MS;
        }
    }
    unsafe {
        delay_tick((hours as u64 * 3600000 + minutes as u64 * 60000 + seconds as u64 * 1000 + ms) * TICK_HZ / 1000);
    }
    return OS_ERR_STATE::OS_ERR_NONE;
}

// #[cfg(feature = "OS_TIME_DLY_RESUME_EN")]
/// This function is used resume a task that has been delayed 
/// through a call to either OSTimeDly() or OSTimeDlyHMSM().
pub fn OSTimeDlyResume(prio: INT8U) -> OS_ERR_STATE {
    // #[cfg(feature = "defmt")]
    #[cfg(feature = "alarm_test")]
    trace!("OSTimeDlyResume");

    if prio >= OS_LOWEST_PRIO {
        return OS_ERR_STATE::OS_ERR_PRIO_INVALID;
    }

    let result = critical_section::with(|_| {
        let executor = GlobalSyncExecutor.as_ref().unwrap();
        let prio_tbl = executor.get_prio_tbl();

        let mut _ptcb = prio_tbl[prio as USIZE];
        // the task does not exist
        if _ptcb.ptr.is_none() {
            return OS_ERR_STATE::OS_ERR_TASK_NOT_EXIST;
        }
        unsafe {
            if _ptcb.expires_at.get() == u64::MAX || _ptcb.expires_at.get() < RTC_DRIVER.now() {
                return OS_ERR_STATE::OS_ERR_TIME_NOT_DLY;
            } 
            _ptcb.expires_at.set(u64::MAX);
            executor.enqueue(_ptcb);
            executor.timer_queue.remove(_ptcb);
        }
        return OS_ERR_STATE::OS_ERR_NONE;
    });
    
    if result != OS_ERR_STATE::OS_ERR_NONE {
        return result;
    }

    if OSRunning.load(Ordering::Acquire) {
        unsafe { GlobalSyncExecutor.as_ref().unwrap().IntCtxSW() };
    }

    return OS_ERR_STATE::OS_ERR_NONE;
}

/// Obtain the current value of the clock ticks since OS boot.
#[cfg(feature = "OS_TIME_GET_SET_EN")]
pub fn OSTimeGet() -> INT64U {
    #[cfg(feature = "defmt")]
    trace!("OSTimeGet");
    RTC_DRIVER.now() 
}

