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
//! Raw task storage and pool.
//! The executor for the uC/OS-II RTOS.

#[cfg_attr(feature = "cortex_m", path = "./state_atomics_arm.rs")]
pub mod state;
pub mod timer_queue;
pub mod waker;
pub mod task;
pub mod mem;
pub mod os_task;
pub mod os_core;
pub mod os_cpu;
pub mod os_time;

pub extern crate alloc;
use core::alloc::Layout;
use core::ptr::NonNull;
use core::sync::atomic::Ordering;

#[cfg(any(feature = "alarm_test", feature = "defmt"))]
use defmt::{trace,info};
use lazy_static::lazy_static;
use state::State;
use port::time_driver::{AlarmHandle, Driver, RTC_DRIVER};
use task::{OS_TCB, OS_TCB_REF};

pub use os_task::*;
pub use os_core::*;
pub use crate::waker::task_from_waker;

use crate::mem::heap::{alloc_stack, OS_STK_REF, PROGRAM_STACK, TASK_STACK_SIZE};

#[cfg(feature = "delay_idle")]
use crate::os_time::blockdelay::delay;

use app::led::{stack_pin_high, stack_pin_low};
use cell::SyncUnsafeCell;
use cfg::*;
use cfg::ucosii::*;
// use port::*;

#[cfg(feature = "unstable-pac")]
pub use stm32_metapac as pac;
#[cfg(not(feature = "unstable-pac"))]
pub(crate) use stm32_metapac as pac;

/*
****************************************************************************************************************************************
*                                                             global variables
****************************************************************************************************************************************
*/
// create a global executor
lazy_static! {
/// the global executor will be initialized at os init
    pub(crate) static ref GlobalSyncExecutor: Option<SyncExecutor> = Some(SyncExecutor::new());
}
/*
****************************************************************************************************************************************
*                                                             type define
****************************************************************************************************************************************
*/


/// The executor for the uC/OS-II RTOS.
pub(crate) struct SyncExecutor {
    // run_queue: RunQueue,
    // the prio tbl stores a relation between the prio and the task_ref
    os_prio_tbl: SyncUnsafeCell<[OS_TCB_REF; (OS_LOWEST_PRIO + 1) as usize]>,
    // indicate the current running task
    pub(crate) OSPrioCur: SyncUnsafeCell<OS_PRIO>,
    pub(crate) OSTCBCur: SyncUnsafeCell<OS_TCB_REF>,
    // highest priority task in the ready queue
    pub(crate) OSPrioHighRdy: SyncUnsafeCell<OS_PRIO>,
    pub(crate) OSTCBHighRdy: SyncUnsafeCell<OS_TCB_REF>,
    // by liam: add a bitmap to record the status of the task
    #[cfg(feature = "OS_PRIO_LESS_THAN_64")]
    OSRdyGrp: SyncUnsafeCell<u8>,
    #[cfg(feature = "OS_PRIO_LESS_THAN_64")]
    OSRdyTbl: SyncUnsafeCell<[u8; OS_RDY_TBL_SIZE]>,
    #[cfg(feature = "OS_PRIO_LESS_THAN_256")]
    OSRdyGrp: u16,
    #[cfg(feature = "OS_PRIO_LESS_THAN_256")]
    OSRdyTbl: [u16; OS_RDY_TBL_SIZE],
    pub(crate) timer_queue: timer_queue::TimerQueue,
    pub(crate) alarm: AlarmHandle,
}

impl SyncExecutor {
    /// The global executor for the uC/OS-II RTOS.
    pub(crate) fn new() -> Self {
        let alarm = unsafe { RTC_DRIVER.allocate_alarm().unwrap() };
        Self {
            os_prio_tbl: SyncUnsafeCell::new([OS_TCB_REF::default(); (OS_LOWEST_PRIO + 1) as usize]),

            OSPrioCur: SyncUnsafeCell::new(OS_TASK_IDLE_PRIO),
            OSTCBCur: SyncUnsafeCell::new(OS_TCB_REF::default()),

            OSPrioHighRdy: SyncUnsafeCell::new(OS_TASK_IDLE_PRIO),
            OSTCBHighRdy: SyncUnsafeCell::new(OS_TCB_REF::default()),

            OSRdyGrp: SyncUnsafeCell::new(0),
            OSRdyTbl: SyncUnsafeCell::new([0; OS_RDY_TBL_SIZE]),
            timer_queue: timer_queue::TimerQueue::new(),
            alarm,
        }
    }
    
    /// set the current to be highrdy
    pub(crate) unsafe fn set_cur_highrdy(&self) {
        #[cfg(feature = "defmt")]
        trace!("set_cur_highrdy");
        self.OSPrioCur.set(self.OSPrioHighRdy.get());
        self.OSTCBCur.set(self.OSTCBHighRdy.get());
    }

    /// Enqueue a task in the task queue
    #[inline(always)]
    pub unsafe fn enqueue(&self, task: OS_TCB_REF) {
        // according to the priority of the task, we place the task in the right place of os_prio_tbl
        // also we will set the corresponding bit in the OSRdyTbl and OSRdyGrp
        let prio = task.OSTCBPrio as usize;
        let tmp = self.OSRdyGrp.get_mut();
        *tmp = *tmp | task.OSTCBBitY;
        let tmp = self.OSRdyTbl.get_mut();
        tmp[task.OSTCBY as usize] |= task.OSTCBBitX;
        // set the task in the right place of os_prio_tbl
        let tmp = self.os_prio_tbl.get_mut();
        tmp[prio] = task;
    }

    pub(crate) unsafe fn set_highrdy(&self) {
        #[cfg(feature = "defmt")]
        trace!("set_highrdy");
        let tmp = self.OSRdyGrp.get_unmut();
        // if there is no task in the ready queue, return None also set the current running task to the lowest priority
        if *tmp == 0 {
            self.OSPrioHighRdy.set(OS_TASK_IDLE_PRIO);
            self.OSTCBHighRdy
                .set(self.os_prio_tbl.get_unmut()[OS_TASK_IDLE_PRIO as usize]);
            return;
        }
        let prio = tmp.trailing_zeros() as usize;
        let tmp = self.OSRdyTbl.get_unmut();
        let prio = prio * 8 + tmp[prio].trailing_zeros() as usize;
        // set the current running task
        self.OSPrioHighRdy.set(prio as OS_PRIO);
        self.OSTCBHighRdy.set(self.os_prio_tbl.get_unmut()[prio]);
    }
    pub(crate) unsafe fn set_highrdy_with_prio(&self, prio: OS_PRIO) {
        // set the current running task
        self.OSPrioHighRdy.set(prio as OS_PRIO);
        self.OSTCBHighRdy.set(self.os_prio_tbl.get_unmut()[prio as usize]);
    }
    pub(crate) fn find_highrdy_prio(&self) -> OS_PRIO {
        #[cfg(feature = "defmt")]
        trace!("find_highrdy_prio");
        let tmp = self.OSRdyGrp.get_unmut();
        if *tmp == 0 {
            return OS_TASK_IDLE_PRIO;
        }
        let prio = tmp.trailing_zeros() as usize;
        let tmp = self.OSRdyTbl.get_unmut();
        let prio = prio * 8 + tmp[prio].trailing_zeros() as usize;
        prio as OS_PRIO
    }
    pub unsafe fn set_task_unready(&self, task: OS_TCB_REF) {
        #[cfg(feature = "defmt")]
        trace!("set_task_unready");
        // added by liam: we have to make this process in critical section
        // because the bitmap is shared by all the tasks
        critical_section::with(|_| {
            let tmp = self.OSRdyTbl.get_mut();
            tmp[task.OSTCBY as usize] &= !task.OSTCBBitX;
            // when the group is empty, we need to set the corresponding bit in the OSRdyGrp to 0
            if tmp[task.OSTCBY as usize] == 0 {
                let tmp = self.OSRdyGrp.get_mut();
                *tmp &= !task.OSTCBBitY;
            }
        });
    }
    // check if an prio is exiting
    pub extern "aapcs" fn prio_exist(&self, prio: INT8U) -> bool {
        let prio_tbl: &[OS_TCB_REF; (OS_LOWEST_PRIO + 1) as usize];
        prio_tbl = self.os_prio_tbl.get_unmut();
        prio_tbl[prio as USIZE].ptr.is_some()
    }
    // to take up space in the bitmap
    pub extern "aapcs" fn reserve_bit(&self, prio: INT8U) {
        let prio_tbl: &mut [OS_TCB_REF; (OS_LOWEST_PRIO + 1) as usize];
        prio_tbl = self.os_prio_tbl.get_mut();
        // use the dangling pointer(Some) to reserve the bit
        prio_tbl[prio as USIZE].ptr = Some(NonNull::dangling());
    }
    pub extern "aapcs" fn clear_bit(&self, prio: INT8U) {
        let prio_tbl: &mut [OS_TCB_REF; (OS_LOWEST_PRIO + 1) as usize];
        prio_tbl = self.os_prio_tbl.get_mut();
        // use the dangling pointer(Some) to reserve the bit
        prio_tbl[prio as USIZE].ptr = None;
    }

    // by noah:TEST print the ready queue
    #[cfg(feature = "defmt")]
    pub fn print_ready_queue(&self) {
        let tmp: [u8; OS_RDY_TBL_SIZE];
        unsafe {
            tmp = self.OSRdyTbl.get();
        }
        {
            info!("the ready queue is:");
            for i in 0..OS_LOWEST_PRIO + 1 {
                if tmp[(i / 8) as usize] & (1 << (i % 8)) != 0 {
                    info!("the {}th task is ready", i);
                }
            }
        }
    }
    
    #[cfg(feature = "OS_TASK_NAME_EN")]
    /// set task's name
    pub fn set_name(&self, prio: INT8U, name: String) {
        let prio_tbl = self.os_prio_tbl.get_mut();
        prio_tbl[prio as usize].OSTCBTaskName = name;
    }

    pub fn get_prio_tbl(&self) -> &[OS_TCB_REF; (OS_LOWEST_PRIO + 1) as usize] {
        self.os_prio_tbl.get_unmut()
    }
}

impl SyncExecutor {
    fn alarm_callback(ctx: *mut ()) {
        #[cfg(feature = "defmt")]
        trace!("alarm_callback");
        #[cfg(feature = "alarm_test")]
        info!("alarm_callback task");
        let this: &Self = unsafe { &*(ctx as *const Self) };
        // first to dequeue all the expired task, note that there must
        // have a task in the tiemr_queue because the alarm is triggered
        loop {
            unsafe { this.timer_queue.dequeue_expired(RTC_DRIVER.now(), wake_task_no_pend) };
            // then we need to set a new alarm according to the next expiration time
            let next_expire = unsafe { this.timer_queue.next_expiration() };
            // by noah：we also need to updater the set_time of the timer_queue
            unsafe {
                this.timer_queue.set_time.set(next_expire);
            }
            if RTC_DRIVER.set_alarm(this.alarm, next_expire) {
                break;
            }
        }
        // call Interrupt Context Switch
        unsafe { this.IntCtxSW() };
    }
    // as an interface to join the scheduler logic
    pub(crate) unsafe fn IntCtxSW(&'static self) {
        #[cfg(feature = "alarm_test")]
        info!("IntCtxSW");
        stack_pin_high();
        // set the cur task's is_in_thread_poll to false, as it is preempted in the interrupt context
        #[cfg(feature = "defmt")]
        info!("IntCtxSW");
        if critical_section::with(|_| unsafe {
            let new_prio = self.find_highrdy_prio();
            #[cfg(feature = "defmt")]
            trace!(
                " the new_prio is {}, the highrdy task's prio is {}, the cur task's prio is {}",
                new_prio,
                self.OSPrioHighRdy.get_unmut(),
                self.OSTCBCur.get_unmut().OSTCBPrio
            );
            if new_prio >= self.OSPrioCur.get() {
                #[cfg(feature = "defmt")]
                trace!("no need to switch task");
                false
            } else {
                // If the new task has a higher priority than the current task 
                // and is not on interrupt as well as does not have a scheduling lock, we need to switch the task
                if OSIntNesting.load(Ordering::Acquire) == 0{
                    if OSLockNesting.load(Ordering::Acquire) == 0{
                        #[cfg(feature = "defmt")]
                        trace!("need to switch task");
                        self.set_highrdy_with_prio(new_prio);

                        return true;
                    }
                }
                false
            }
        }) 
        {
            unsafe { self.interrupt_poll() }
        }
        stack_pin_low();
    }

    /// this function must be called in the interrupt context, and it will trigger pendsv to switch the task
    /// when this function return, the caller interrupt will also return and the pendsv will run.
    pub(crate) unsafe fn interrupt_poll(&'static self) {
        extern "Rust" {
            fn OSTaskStkInit(stk_ref: NonNull<OS_STK>) -> NonNull<OS_STK>;
            fn restore_thread_task();
        }
        // test: print the ready queue
        #[cfg(feature = "defmt")]
        critical_section::with(|_| {
            info!("in interrupt_poll");
            self.print_ready_queue();
            info!("the highrdy task's prio is {}", self.OSPrioHighRdy.get_unmut());
        });

        #[cfg(feature = "defmt")]
        trace!("interrupt_poll");
        if *self.OSPrioCur.get_unmut() != OS_TASK_IDLE_PRIO {
            self.OSTCBCur.get().is_in_thread_poll.set(false);
            // If the current task will be deleted, 
            // setting 'is_in_thread_poll' to 'true' will destroy the stack in PenSV
            if self.os_prio_tbl.get_unmut()[*self.OSPrioCur.get_unmut() as usize].ptr.is_none() {
                self.OSTCBCur.get().is_in_thread_poll.set(true);
            }
        }
        let mut task = critical_section::with(|_| self.OSTCBHighRdy.get());

        // then we need to restore the highest priority task
        #[cfg(feature = "defmt")]
        {
            trace!("interrupt poll :the highrdy task's prio is {}", task.OSTCBPrio);
            trace!("interrupt poll :the cur task's prio is {}", self.OSPrioCur.get_unmut());
        }
        #[cfg(feature = "alarm_test")]
        {
            info!("the current task is {}", *self.OSPrioCur.get_unmut());
            // info!("alloc stack for the task {}", *self.OSPrioHighRdy.get_unmut());
        }
        if task.OSTCBStkPtr.is_none() {
            #[cfg(feature = "alarm_test")]
            // #[cfg(feature = "defmt")]
            info!("the task's stk is none");
            // if the task has no stack, it's a task, we need to mock a stack for it.
            // we need to alloc a stack for the task

            // by noah: *TEST*. Maybe when alloc_stack is called, we need the cs
            let mut stk: OS_STK_REF;
            if *self.OSPrioCur.get_unmut() == OS_TASK_IDLE_PRIO {
                #[cfg(feature = "alarm_test")]
                info!("the current task is idle");             
                // if is idle, we don't need to alloc stack,just use the idle stack
                // by yck: but this branch will not be executed
                let mut program_stk = PROGRAM_STACK.exclusive_access();
                program_stk.STK_REF = NonNull::new(
                    program_stk.HEAP_REF.as_ptr().offset(program_stk.layout.size() as isize) as *mut OS_STK,
                )
                .unwrap();
                stk = program_stk.clone();
            } else {
                #[cfg(feature = "alarm_test")]
                {
                    // info!("the current task is {}", *self.OSPrioCur.get_unmut());
                    info!("alloc stack for the prio {} task", *self.OSPrioHighRdy.get_unmut());
                }
                let layout = Layout::from_size_align(TASK_STACK_SIZE, 4).unwrap();
                stk = alloc_stack(layout);
                #[cfg(feature = "alarm_test")]
                {
                    info!("the bottom of the allocated stk is {:?}", stk.STK_REF);
                }
            }
            // then we need to mock the stack for the task(the stk will change during the mock)
            stk.STK_REF = OSTaskStkInit(stk.STK_REF);

            task.OSTCBStkPtr = Some(stk);
        } else {
            #[cfg(feature = "alarm_test")]
            {
                info!("the highrdy task {} have a stack {}", *self.OSPrioHighRdy.get_unmut(), task.OSTCBStkPtr.as_ref().unwrap().STK_REF);
            }
        }
        // restore the task from stk
        critical_section::with(|_| {
            if task.OSTCBPrio == *self.OSPrioHighRdy.get_unmut() {
                unsafe {
                    // #[cfg(feature = "defmt")]
                    #[cfg(feature = "alarm_test")]
                    info!("restore the task/thread");
                    restore_thread_task()
                };
            }
        });
    }

    /// since when it was called, there is no task running, we need poll all the task that is ready in bitmap
    pub(crate) unsafe fn poll(&'static self) -> ! {
        // #[cfg(feature = "defmt")]
        #[cfg(feature = "alarm_test")]
        trace!("poll");
        RTC_DRIVER.set_alarm_callback(self.alarm, Self::alarm_callback, self as *const _ as *mut ());
        // build this as a loop
        loop {
            // test: print the ready queue
            #[cfg(feature = "defmt")]
            critical_section::with(|_| {
                info!("in poll");
                self.print_ready_queue();
                info!("the highrdy task's prio is {}", self.OSPrioHighRdy.get_unmut());
            });
            // if the highrdy task is the idle task, we need to delay some time
            #[cfg(feature = "delay_idle")]
            if critical_section::with(|_| *self.OSPrioHighRdy.get_unmut() == OS_TASK_IDLE_PRIO) {
                #[cfg(feature = "defmt")]
                trace!("begin delay the idle task");
                delay(block_delay_poll);
                #[cfg(feature = "defmt")]
                trace!("end delay the idle task");
            }
            // in the executor's thread poll, the highrdy task must be polled, there we don't set cur to be highrdy
            let task = critical_section::with(|_| {
                let mut task = self.OSTCBHighRdy.get();
                if task.OSTCBStkPtr.is_none() {
                    self.OSPrioCur.set(task.OSTCBPrio);
                    self.OSTCBCur.set(task);
                } else {
                    // if the task has stack, it's a thread, we need to resume it not poll it
                    #[cfg(feature = "defmt")]
                    {
                        trace!("resume the task");
                        trace!("the highrdy task's prio is {}", task.OSTCBPrio);
                    }
                    task.restore_context_from_stk();
                    return None;
                }
                Some(task)
            });
            if task.is_none() {
                continue;
            }
            let task = task.unwrap();
            // execute the task depending on if it has stack
            self.single_poll(task);
        }
    }

    pub unsafe fn single_poll(&'static self,mut task: OS_TCB_REF) {
        #[cfg(feature = "alarm_test")]
        trace!("single_poll");
        task.OS_POLL_FN.get().unwrap_unchecked()(task);
            // by noah：Remove tasks from the ready queue in advance to facilitate subsequent unified operations
            // update timer
            // by yck: but the following part will not be executed, because OS_POLL_FN will execute task's 'poll', 
            // which in turn will go to the task body, and will not return here
            critical_section::with(|_| {
                task.is_in_thread_poll.set(true);
                self.timer_queue.dequeue_expired(RTC_DRIVER.now(), wake_task_no_pend);
                self.set_task_unready(task);
                // set the task's stack to None
                // check: this seems no need to set it to None as it will always be None
                task.OSTCBStkPtr = None;
                let mut next_expire = self.timer_queue.update(task);
                if next_expire < *self.timer_queue.set_time.get_unmut() {
                    self.timer_queue.set_time.set(next_expire);
                    // by noah：if the set alarm return false, it means the expire arrived.
                    // So we can not set the **task which is waiting for the next_expire** as unready
                    // The **task which is waiting for the next_expire** must be current task
                    // we must do this until we set the alarm successfully or there is no alarm required
                    while !RTC_DRIVER.set_alarm(self.alarm, next_expire) {
                        #[cfg(feature = "defmt")]
                        trace!("the set alarm return false");
                        // by noah: if set alarm failed, it means the expire arrived, so we should not set the task unready
                        // we should **dequeue the task** from time_queue, **clear the set_time of the time_queue** and continue the loop
                        // (just like the operation in alarm_callback)
                        self.timer_queue.dequeue_expired(RTC_DRIVER.now(), wake_task_no_pend);
                        // then we need to set a new alarm according to the next expiration time
                        next_expire = unsafe { self.timer_queue.next_expiration() };
                        // by noah：we also need to updater the set_time of the timer_queue
                        unsafe {
                            self.timer_queue.set_time.set(next_expire);
                        }
                    }
                }
                // by noah：maybe we can set the task unready, and call dequeue when set_alarm return false
                // find the highest priority task in the ready queue
                // adapt the method above
                self.set_highrdy()
            });
    }
    
}

/*
****************************************************************************************************************************************
*                                                           function define
****************************************************************************************************************************************
*/

/// Wake a task by `TaskRef`.
///
/// You can obtain a `TaskRef` from a `Waker` using [`task_from_waker`].
pub fn wake_task(task: OS_TCB_REF) {
    #[cfg(feature = "defmt")]
    trace!("wake_task");
    let header = task.header();
    if header.OSTCBStat.run_enqueue() {
        // We have just marked the task as scheduled, so enqueue it.
        unsafe {
            let executor = GlobalSyncExecutor.as_ref().unwrap_unchecked();
            executor.enqueue(task);
        }
    }
}

/// Wake a task by `TaskRef`.
pub fn wake_task_no_pend(task: OS_TCB_REF) {
    #[cfg(feature = "defmt")]
    trace!("wake_task_no_pend");
    // We have just marked the task as scheduled, so enqueue it.
    unsafe {
        let executor = GlobalSyncExecutor.as_ref().unwrap();
        executor.enqueue(task);
    }
}

#[no_mangle]
/// Schedule the given waker to be woken at `at`.
pub fn _embassy_time_schedule_wake(at: u64, waker: &core::task::Waker) {
    #[cfg(feature = "defmt")]
    trace!("_embassy_time_schedule_wake");
    let task = waker::task_from_waker(waker);
    let task = task.header();
    unsafe {
        let expires_at = task.expires_at.get();
        task.expires_at.set(expires_at.min(at));
    }
}