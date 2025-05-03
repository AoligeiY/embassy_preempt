//! task management
/*
********************************************************************************************************************************************
*                                                           TASK MANAGEMENT
*                                           provide interface about the task of uC/OS-II kernel
********************************************************************************************************************************************
*/

/*
********************************************************************************************************************************************
*                                                           import
********************************************************************************************************************************************
*/

use alloc::string::ToString;
use core::alloc::Layout;
use core::ffi::c_void;
use core::future::Future;
use core::sync::atomic::Ordering;

use super::{GlobalSyncExecutor, OS_TCB_REF, task::OS_TASK_STORAGE};

use crate::cfg::{OS_LOWEST_PRIO, OS_TASK_REG_TBL_SIZE};
use crate::executor::mem::heap::{dealloc_stack, stk_from_ptr};
use crate::port::{INT8U, USIZE, OS_STK};
use crate::cfg::ucosii::{OS_PRIO_SELF, OS_TASK_IDLE_PRIO, OSRunning, OSIntNesting, OSTaskCtr, OS_ERR_STATE};

#[cfg(feature = "defmt")]
#[allow(unused)]
use defmt::{trace,info};
#[cfg(feature = "alarm_test")]
#[allow(unused)]
use defmt::{trace,info};


const DEFAULT_REVOKE_STACK_SIZE: usize = 128;

/*
********************************************************************************************************************************************
*                                                           interface
********************************************************************************************************************************************
*/
/// the trait to check whether the return type is unit or never return
pub trait ReturnUnitOrNeverReturn {}

impl ReturnUnitOrNeverReturn for ! {}
impl ReturnUnitOrNeverReturn for () {}

/// Create a task in uC/OS-II kernel. This func is used by C
// _ptos is not used in this func, because stack allocation is done by the stack allocator when scheduling
pub extern "aapcs" fn SyncOSTaskCreate<F, R>(
    task: F,
    p_arg: *mut c_void,
    _ptos: *mut OS_STK,
    prio: INT8U,
) -> OS_ERR_STATE
where
    // check by liam: why the future is 'static: because the definition of OS_TASK_STORAGE's generic F is 'static
    F: FnOnce(*mut c_void) -> R + 'static,
    R: ReturnUnitOrNeverReturn,
{
    #[cfg(feature = "defmt")]
    trace!("SyncOSTaskCreate");
    // check the priority
    if prio > OS_LOWEST_PRIO as u8 {
        return OS_ERR_STATE::OS_ERR_PRIO_INVALID;
    }
    // warp the normal func to a async func
    let future_func = move || async move { task(p_arg) };
    #[cfg(feature = "defmt")]
    trace!("the size of future is {}", core::mem::size_of_val(&future_func));
    // if the ptos is not null, we will revoke it as the miniaml stack size(which is 128 B)
    if !_ptos.is_null() {
        let layout = Layout::from_size_align(DEFAULT_REVOKE_STACK_SIZE, 4).unwrap();
        let heap_ptr = unsafe { (_ptos as *mut u8).offset(-(DEFAULT_REVOKE_STACK_SIZE as isize)) };
        // by noah: used to test ffi
        #[cfg(feature = "defmt")]
        trace!("Task Create");
        let mut stk = stk_from_ptr(heap_ptr as *mut u8, layout);
        dealloc_stack(&mut stk);
    }
    OSTaskCtr.fetch_add(1, Ordering::SeqCst);
    return init_task(prio, future_func);
}

/// Create a task in uC/OS-II kernel. This func is used by async Rust
pub fn AsyncOSTaskCreate<F, FutFn>(task: FutFn, p_arg: *mut c_void, _ptos: *mut OS_STK, prio: INT8U) -> OS_ERR_STATE
where
    // check by liam: why the future is 'static: because the definition of OS_TASK_STORAGE's generic F is 'static
    F: Future + 'static,
    FutFn: FnOnce(*mut c_void) -> F + 'static,
{
    #[cfg(feature = "defmt")]
    trace!("AsyncOSTaskCreate");
    let future_func = || task(p_arg);
    // if the ptos is not null, we will revoke it as the miniaml stack size(which is 128 B)
    if !_ptos.is_null() {
        let layout = Layout::from_size_align(DEFAULT_REVOKE_STACK_SIZE, 4).unwrap();
        let heap_ptr = unsafe { (_ptos as *mut u8).offset(-(DEFAULT_REVOKE_STACK_SIZE as isize)) };
        let mut stk = stk_from_ptr(heap_ptr as *mut u8, layout);
        dealloc_stack(&mut stk);
    }
    OSTaskCtr.fetch_add(1, Ordering::SeqCst);
    return init_task(prio, future_func);
}


#[no_mangle]
/// helper func
pub extern "aapcs" fn OSTaskCreate(
    fun_ptr: extern "aapcs" fn(*mut c_void),
    p_arg: *mut c_void,
    ptos: *mut OS_STK,
    prio: INT8U,
) -> OS_ERR_STATE {
    #[cfg(feature = "defmt")]
    trace!("OSTaskCreate");
    let fun_ptr = move |p_arg| fun_ptr(p_arg);
    SyncOSTaskCreate(fun_ptr, p_arg, ptos, prio)
}

fn init_task<F: Future + 'static>(prio: INT8U, future_func: impl FnOnce() -> F) -> OS_ERR_STATE {
    // Make sure we don't create the task from within an ISR
    if OSIntNesting.load(Ordering::Acquire) > 0 {
        return OS_ERR_STATE::OS_ERR_TASK_CREATE_ISR;
    }
    // because this func can be call when the OS has started, so need a cs
    if critical_section::with(|_cs| {
        let executor = GlobalSyncExecutor.as_ref().unwrap();
        if executor.prio_exist(prio) {
            return true;
        } else {
            // reserve bit
            executor.reserve_bit(prio);
            return false;
        }
    }) {
        #[cfg(feature = "defmt")]
        trace!("the prio is exist");
        return OS_ERR_STATE::OS_ERR_PRIO_EXIST;
    }

    let err = OS_TASK_STORAGE::init(prio, 0, 0 as *mut (), 0, "".to_string(), future_func);
    if err == OS_ERR_STATE::OS_ERR_NONE {
        // check whether the task is created after the OS has started
        if OSRunning.load(Ordering::Acquire) {
            // schedule the task, not using poll, we have to make a preemptive schedule
            unsafe {
                GlobalSyncExecutor.as_ref().unwrap().IntCtxSW();
            }
        }
    } else {
        critical_section::with(|_cs| {
            let executor = GlobalSyncExecutor.as_ref().unwrap();
            // clear the reserve bit
            executor.clear_bit(prio);
        })
    }
    return err;
}


/*
****************************************************************************************************************************************
*                                                           function define
****************************************************************************************************************************************
*/

// #[cfg(feature = "OS_TASK_CHANGE_PRIO_EN")]
/// This function allows you to change the priority of a task dynamically.  
/// Note that the new priority MUST be available.
pub fn OSTaskChangePrio(old_prio: INT8U, new_prio:INT8U) -> OS_ERR_STATE {
    #[cfg(feature = "alarm_test")]
    trace!("OSTaskChangePrio");
    let mut old_prio = old_prio;
    let executor = GlobalSyncExecutor.as_ref().unwrap();
    #[cfg(feature = "OS_ARG_CHK_EN")]
    {
        if old_prio >= OS_LOWEST_PRIO {
            if old_prio != OS_PRIO_SELF as u8 {
                return OS_ERR_STATE::OS_ERR_PRIO_INVALID;
            }
        }
        if new_prio >= OS_LOWEST_PRIO {
            return OS_ERR_STATE::OS_ERR_PRIO_INVALID;
        }
    }
    
    let result = critical_section::with(|_| {
        let prio_tbl: &mut [OS_TCB_REF; (OS_LOWEST_PRIO + 1) as usize];
        prio_tbl = executor.os_prio_tbl.get_mut();
        // check if the new prio is exist
        if prio_tbl[new_prio as USIZE].ptr.is_some() {
            return OS_ERR_STATE::OS_ERR_PRIO_EXIST;
        }
        // the OSPrioCur is only valid after os has started
        if OSRunning.load(Ordering::Acquire) {
            // see if changing self
            if old_prio == OS_PRIO_SELF as u8 {
                old_prio = *executor.OSPrioCur.get_unmut();
            }
        }
        // Does task to change exist?
        if !prio_tbl[old_prio as USIZE].ptr.is_some() {
            return OS_ERR_STATE::OS_ERR_PRIO;
        }
        let mut _ptcb = prio_tbl[old_prio as USIZE];

        if OSRunning.load(Ordering::Acquire) {
            // if current task change prio itself, must set OSPrioCur to new prio
            if old_prio == *executor.OSPrioCur.get_unmut() { 
                unsafe { executor.OSPrioCur.set(new_prio); }
            }
        }

        // new priority's bitmap
        let y_new = new_prio >> 3;
        let x_new = new_prio & 0x07;
        let bity_new = 1 << y_new;
        let bitx_new = 1 << x_new;
        // remove the old priority from the priority table 
        // and place it in the new priority in the priority table
        prio_tbl[old_prio as USIZE].ptr = None;
        prio_tbl[new_prio as USIZE] = _ptcb;

        let y_old = _ptcb.OSTCBY;
        let bity_old = _ptcb.OSTCBBitY;
        let bitx_old = _ptcb.OSTCBBitX;
        // bitmap
        let os_rdy_tbl = executor.OSRdyTbl.get_mut();
        let os_rdy_grp = executor.OSRdyGrp.get_mut();
        // remove the old priority from the ready queue
        if os_rdy_tbl[y_old as USIZE] & bitx_old != 0 {
            os_rdy_tbl[y_old as USIZE] &= !bitx_old;
            if os_rdy_tbl[y_old as USIZE] == 0 {
                *os_rdy_grp &= !bity_old;
            }
            // add new priority to the ready queue
            *os_rdy_grp |= bity_new;
            os_rdy_tbl[y_new as USIZE] |= bitx_new;
        }
        // update the tcb's priority to the new priority
        _ptcb.OSTCBPrio = new_prio;
        _ptcb.OSTCBY = y_new;
        _ptcb.OSTCBX = x_new;
        _ptcb.OSTCBBitY = bity_new;
        _ptcb.OSTCBBitX = bitx_new;

        OS_ERR_STATE::OS_ERR_NONE
    });
    if result != OS_ERR_STATE::OS_ERR_NONE {
        return result;
    }
    if OSRunning.load(Ordering::Acquire) {
        unsafe { GlobalSyncExecutor.as_ref().unwrap().IntCtxSW() };
    }
    return OS_ERR_STATE::OS_ERR_NONE;
}

// #[cfg(feature = "OS_TASK_DEL_EN")]
/// this function allows you to delete a task 
pub fn OSTaskDel(prio: INT8U) -> OS_ERR_STATE {
    #[cfg(feature = "alarm_test")]
    trace!("OSTaskDel");
    
    let mut prio = prio;
    // See if trying to delete from ISR 
    if OSIntNesting.load(Ordering::Acquire) > 0 {
        return OS_ERR_STATE::OS_ERR_TASK_DEL_ISR;
    }
    // Not allowed to delete idle task
    if prio == OS_TASK_IDLE_PRIO {
        return OS_ERR_STATE::OS_ERR_TASK_DEL_IDLE;
    }
    #[cfg(feature = "OS_ARG_CHK_EN")]
    {
        // whether a task priority is valid
        if prio >= OS_LOWEST_PRIO {
            if prio != OS_PRIO_SELF as u8 {
                return OS_ERR_STATE::OS_ERR_PRIO_INVALID;
            }
        }
    }
    let result = critical_section::with(|_| {
        let executor = GlobalSyncExecutor.as_ref().unwrap();
        let prio_tbl: &mut [OS_TCB_REF; (OS_LOWEST_PRIO + 1) as usize];
        prio_tbl = executor.os_prio_tbl.get_mut();

        // See if requesting to delete self
        if prio == OS_PRIO_SELF as u8 {
            // Set priority to delete to current
            prio = *executor.OSPrioCur.get_unmut();
        }
        let mut ptcb = prio_tbl[prio as USIZE];
        // the task does not exist
        if ptcb.ptr.is_none() {
            return OS_ERR_STATE::OS_ERR_TASK_NOT_EXIST;
        }
        let os_rdy_tbl = executor.OSRdyTbl.get_mut();
        let os_rdy_grp = executor.OSRdyGrp.get_mut();
        // remove task from the ready queue
        os_rdy_tbl[ptcb.OSTCBY as USIZE] &= !ptcb.OSTCBBitX;
        if os_rdy_tbl[ptcb.OSTCBY as USIZE] == 0 {
            *os_rdy_grp &= !ptcb.OSTCBBitY;
        }
        // clearing the expiration time of tasks
        unsafe{ ptcb.expires_at.set(u64::MAX); }

        #[cfg(feature = "OS_TASK_REG_TBL_SIZE")]
        if OS_TASK_REG_TBL_SIZE > 0 {
            for i in 0..OS_TASK_REG_TBL_SIZE {
                ptcb.OSTCBRegTbl[i] = 0;
            }
        }
        OSTaskCtr.fetch_sub(1, Ordering::SeqCst);
        ptcb.OSTCBStat.despawn();

        // remove task from the priority table
        prio_tbl[prio as USIZE].ptr = None;
        // destroy stack only when os is running
        if OSRunning.load(Ordering::Acquire) {
            // if prio == executor.OSTCBCur.get_unmut().OSTCBPrio {
            if prio == *executor.OSPrioCur.get_unmut() {
                // deleting the task itself sets 'is_in_thread_poll' to 'true' will destroy the stack in PenSV
                unsafe { ptcb.is_in_thread_poll.set(true); }
            } else {
                // drop the stack directly when deleting other tasks
                dealloc_stack(&mut ptcb.take_stk());
            }
        }
        // remove task from the timer queue
        unsafe { executor.timer_queue.remove(ptcb); }
        #[cfg(feature = "OS_TASK_NAME_EN")]
        {
            ptcb.OSTCBTaskName = "?";
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

#[cfg(feature = "OS_TASK_NAME_EN")]
/// This function is used to set the name of a task.
pub fn OSTaskNameSet(prio: INT8U, pname: &str) -> OS_ERR_STATE {
    // argument checking
    #[cfg(feature = "OS_ARG_CHK_EN")]
    {
        if prio > OS_LOWEST_PRIO {
            return OS_ERR_STATE::OS_ERR_PRIO_INVALID;
        }
        if pname.is_empty() {
            return OS_ERR_STATE::OS_ERR_PNAME_NULL;
        }
    }
    // Make sure we don't set the task's name from within an ISR
    if OSIntNesting.load(Ordering::Acquire) > 0 {
        return OS_ERR_STATE::OS_ERR_NAME_GET_ISR;
    }

    let result = critical_section::with(|_cs| { 
        let executor = GlobalSyncExecutor.as_ref().unwrap();   
        if executor.prio_exist(prio) {
            executor.set_name(prio, pname.to_string());

            OS_ERR_STATE::OS_ERR_NONE
        } else {
            OS_ERR_STATE::OS_ERR_TASK_NOT_EXIST
        }
    });
    return result;
}