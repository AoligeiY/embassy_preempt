use core::sync::atomic::Ordering;

use crate::cfg::ucosii::{OSIntNesting, OS_ERR_STATE, OS_DEL_NO_PEND, OS_DEL_ALWAYS};
use crate::port::{INT8U, INT16U, BOOLEAN};
use crate::executor::GlobalSyncExecutor;
use crate::os_time::OSTimeDly;
use crate::event::{GlobalEventPool, OS_EVENT_REF, OS_EVENT_TYPE};
use crate::event::{OS_EventTaskWait, OS_EventTaskRdy};

/// creates a semaphore
pub fn OSSemCreate(cnt: INT16U) -> Option<OS_EVENT_REF> {
    // See if called from ISR
    if OSIntNesting.load(Ordering::Acquire) > 0 {
        return None;
    }
    let globaleventpool = GlobalEventPool.as_ref().unwrap();
    let pevent = globaleventpool.alloc();
    let mut pevent = pevent.unwrap();

    unsafe {
        // get an event control block
        if pevent.ptr.is_some() {       
            pevent.OSEventType = OS_EVENT_TYPE::SEM;
            // set semaphore value
            pevent.OSEventCnt = cnt;
            // unlink from ECB free list
            pevent.OSEventPtr.set(None);
        }
    }
    // initialize to 'nobody waiting' on sem
    pevent.OS_EventWaitListInit();
    return Some(pevent);
}

// #[cfg(feature = "OS_SEM_ACCEPT_EN")]
/// checks the semaphore to see if a resource is available or, if an event occurred
pub fn OSSemAccept(mut pevent: OS_EVENT_REF) -> INT16U {
    // validate event block type
    if pevent.OSEventType != OS_EVENT_TYPE::SEM {
        return 0;
    }
    let cnt = critical_section::with(|_| {
        let _cnt = pevent.OSEventCnt;
        if _cnt > 0 {
            // decrement semaphore and notify caller
            pevent.OSEventCnt -= 1;
        }
        return _cnt;
    });
    // return semaphore count
    return cnt;
}

// #[cfg(feature = "OS_SEM_DEL_EN")]
/// deletes a semaphore and readies all tasks pending on the semaphore
pub fn OSSemDel(mut pevent: OS_EVENT_REF, opt: INT8U) -> (OS_ERR_STATE, OS_EVENT_REF) {
    let mut pevent_return: OS_EVENT_REF = OS_EVENT_REF::default();
    #[cfg(feature = "OS_ARG_CHK_EN")]
    {
        // validate 'pevent'
        if pevent.ptr.is_none() {
            return (OS_ERR_STATE::OS_ERR_PEVENT_NULL, pevent);
        }
    }
    // Validate event block type
    if pevent.OSEventType != OS_EVENT_TYPE::SEM {
        return (OS_ERR_STATE::OS_ERR_EVENT_TYPE, pevent);
    }
    // see if called from ISR, can't DELETE from an ISR
    if OSIntNesting.load(Ordering::Acquire) > 0 {
        return (OS_ERR_STATE::OS_ERR_DEL_ISR, pevent);
    }
    let mut tasks_waiting: BOOLEAN = false;
    let result = critical_section::with(|_| {
        // See if any tasks waiting on semaphore
        if pevent.OSEventGrp != 0 {
            tasks_waiting = true;
        } else {
            tasks_waiting = false;
        }
        
        match opt as u32 {
            OS_DEL_NO_PEND => {
                if tasks_waiting == false {
                    GlobalEventPool.as_ref().unwrap().free(pevent);
                    pevent.OSEventCnt = 0;
                    pevent_return = OS_EVENT_REF::default();
                    return OS_ERR_STATE::OS_ERR_NONE;
                } else {
                    pevent_return = pevent;
                    return OS_ERR_STATE::OS_ERR_TASK_WAITING;
                }
            }
            OS_DEL_ALWAYS => {
                while pevent.OSEventGrp != 0 {
                    OS_EventTaskRdy(pevent);
                }
                GlobalEventPool.as_ref().unwrap().free(pevent);
                pevent.OSEventCnt = 0;
                // reschedule only if task(s) were waiting
                if tasks_waiting {
                    unsafe {
                        GlobalSyncExecutor.as_ref().unwrap().IntCtxSW();
                    }
                }
                // semaphore has been deleted
                pevent_return = OS_EVENT_REF::default();
                return OS_ERR_STATE::OS_ERR_NONE;
            }
            _ => {
                pevent_return = pevent;
                return OS_ERR_STATE::OS_ERR_INVALID_OPT;
            }
        }
    });
    if result != OS_ERR_STATE::OS_ERR_NONE {
        return (result, pevent_return);
    }
    return (OS_ERR_STATE::OS_ERR_NONE, pevent_return);
}

/// waits for a semaphore
pub fn OSSemPend(mut pevent: OS_EVENT_REF, timeout: INT16U) -> OS_ERR_STATE {
    #[cfg(feature = "OS_ARG_CHK_EN")]
    {
        // validate 'pevent'
        if pevent.ptr.is_none() {
            return OS_ERR_STATE::OS_ERR_PEVENT_NULL;
        }
    }
    // validate event block type
    if pevent.OSEventType != OS_EVENT_TYPE::SEM {
        return OS_ERR_STATE::OS_ERR_EVENT_TYPE;
    }
    // See if called from ISR
    if OSIntNesting.load(Ordering::Acquire) > 0 {
        return OS_ERR_STATE::OS_ERR_PEND_ISR;
    }
    let result = critical_section::with(|_| {
        // decrement semaphore count
        if pevent.OSEventCnt > 0 {
            pevent.OSEventCnt -= 1;
            return OS_ERR_STATE::OS_ERR_NONE;
        }

        // suspend task until event or timeout occurs
        OS_EventTaskWait(pevent);
        // exiting the ready queue, releasing control of the CPU
        OSTimeDly(timeout as u64);

        return OS_ERR_STATE::OS_ERR_NONE;
    });
    return result;
}

/// signals a semaphore
pub fn OSSemPost(mut pevent: OS_EVENT_REF) -> OS_ERR_STATE {
    #[cfg(feature = "OS_ARG_CHK_EN")]
    {
        // validate 'pevent'
        if pevent.ptr.is_none() {
            return OS_ERR_STATE::OS_ERR_PEVENT_NULL;
        }
    }
    // validate event block type
    if pevent.OSEventType != OS_EVENT_TYPE::SEM {
        return OS_ERR_STATE::OS_ERR_EVENT_TYPE;
    }
    let result = critical_section::with(|_| {
        if pevent.OSEventGrp != 0 {
            OS_EventTaskRdy(pevent);       
            unsafe { GlobalSyncExecutor.as_ref().unwrap().IntCtxSW(); } 
            return OS_ERR_STATE::OS_ERR_NONE;
        }
        // make sure semaphore will not overflow
        if pevent.OSEventCnt < 65535 {
            // increment semaphore count to register event
            pevent.OSEventCnt += 1;
            return OS_ERR_STATE::OS_ERR_NONE;
        }
        return OS_ERR_STATE::OS_ERR_SEM_OVF;
    });
    return result;
}