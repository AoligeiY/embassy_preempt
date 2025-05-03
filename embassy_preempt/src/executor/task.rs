//! Task implementation
use alloc::string::String;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::mem;

use super::waker;
use super::State;
use super::GlobalSyncExecutor;

use crate::executor::mem::arena::ARENA;
use crate::executor::mem::heap::OS_STK_REF;
use crate::port::{INT8U, INT16U, INT32U, PTR};
use crate::cfg::ucosii::OS_ERR_STATE;
use crate::cfg::OS_TASK_REG_TBL_SIZE;
use crate::executor::cell::{SyncUnsafeCell, UninitCell};

#[cfg(feature = "alarm_test")]
use defmt::{trace,info};
#[cfg(feature = "defmt")]
use defmt::{trace,info};

/// the TCB of the task. It contains the task's info
#[allow(unused)]
// we put it in executor crate to use "pub(crate)" to make it can be used in the other mod in order to reduce coupling
pub struct OS_TCB {
    // it maybe None
    pub(super)  OSTCBStkPtr: Option<OS_STK_REF>, /* Pointer to current top of stack                         */
    // Task specific extension. If the OS_TASK_CREATE_EXT_EN feature is not active, it will be None
    #[cfg(feature = "OS_TASK_CREATE_EXT_EN")]
    pub(super) OSTCBExtInfo: OS_TCB_EXT,

    pub(super) OSTimerNext: SyncUnsafeCell<Option<OS_TCB_REF>>, /* Pointer to next     TCB in the Timer list                 */
    pub(super) OSTimerPrev: SyncUnsafeCell<Option<OS_TCB_REF>>, /* Pointer to previous TCB in the Timer list                 */

    // the poll fn that will be called by the executor. In the func, a waker will be create.
    pub(super) OS_POLL_FN: SyncUnsafeCell<Option<unsafe fn(OS_TCB_REF)>>,

    #[cfg(feature = "OS_EVENT_EN")]
    pub(super) OSTCBEventPtr: Option<OS_EVENT_REF>, /* Pointer to event control block                */

    #[cfg(any(all(feature = "OS_Q_EN", feature = "OS_MAX_QS"), feature = "OS_MBOX_EN"))]
    pub(super) OSTCBMsg: PTR, /* Message received from OSMboxPost() or OSQPost()         */

    pub(super) OSTCBDly: INT32U, /* Nbr ticks to delay task or, timeout waiting for event   */
    pub(super) OSTCBStat: State, /* Task      status                                        */
    
    pub(super) OSTCBPrio: INT8U, /* Task priority (0 == highest)                            */

    pub(super) OSTCBX: INT8U,    /* Bit position in group  corresponding to task priority   */
    pub(super) OSTCBY: INT8U,    /* Index into ready table corresponding to task priority   */
    pub(super) OSTCBBitX: INT8U, /* Bit mask to access bit position in ready table          */
    pub(super) OSTCBBitY: INT8U, /* Bit mask to access bit position in ready group          */

    #[cfg(feature = "OS_TASK_DEL_EN")]
    OSTCBDelReq: INT8U, /* Indicates whether a task needs to delete itself         */

    #[cfg(feature = "OS_TASK_PROFILE_EN")]
    pub(crate) OSTCBCtxSwCtr: INT32U,            /* Number of time the task was switched in                 */
    #[cfg(feature = "OS_TASK_PROFILE_EN")]
    OSTCBCyclesTot: INT32U,           /* Total number of clock cycles the task has been running  */
    #[cfg(feature = "OS_TASK_PROFILE_EN")]
    OSTCBCyclesStart: INT32U,         /* Snapshot of cycle counter at start of task resumption   */
    #[cfg(feature = "OS_TASK_PROFILE_EN")]
    OSTCBStkBase: Option<OS_STK_REF>, /* Pointer to the beginning of the task stack              */
    #[cfg(feature = "OS_TASK_PROFILE_EN")]
    OSTCBStkUsed: INT32U,             /* Number of bytes used from the stack                     */
    
    #[cfg(feature = "OS_TASK_NAME_EN")]
    pub(super) OSTCBTaskName: String,
    
    #[cfg(feature = "OS_TASK_REG_TBL_SIZE")]
    pub(super) OSTCBRegTbl: [INT32U; OS_TASK_REG_TBL_SIZE],

    pub(crate) expires_at: SyncUnsafeCell<u64>,     /* Time when the task should be woken up */
    pub(crate) is_in_thread_poll: SyncUnsafeCell<bool>,     /* Marking whether a task is a thread or a concatenation */
}

#[cfg(feature = "OS_TASK_CREATE_EXT_EN")]
#[allow(unused)]
pub(crate) struct OS_TCB_EXT {
    OSTCBExtPtr: PTR,                   /* Pointer to user definable data for TCB extension        */
    OSTCBStkBottom: Option<OS_STK_REF>, /* Pointer to bottom of stack                              */
    OSTCBStkSize: INT32U,               /* Size of task stack (in number of stack elements)        */
    OSTCBOpt: INT16U,                   /* Task options as passed by OSTaskCreateExt()             */
    OSTCBId: INT16U,                    /* Task ID (0..65535)                                      */
}

/// the storage of the task. It contains the task's TCB and the future
#[allow(unused)]
pub struct OS_TASK_STORAGE<F: Future + 'static> {
    task_tcb: OS_TCB,
    // this part is invisible to other crate
    // by noah: maybe we need to use raw ptr
    future: UninitCell<F>,
}

/// the ref of the TCB. In other crate only it can be used to access the TCB
#[derive(PartialEq)]
#[derive(Clone, Copy)]
#[allow(unused)]
pub struct OS_TCB_REF {
    /// the pointer to the TCB
    pub ptr: Option<NonNull<OS_TCB>>,
}

/// An uninitialized [`OS_TASK_STORAGE`].
#[allow(unused)]
pub struct AvailableTask<F: Future + 'static> {
    task: &'static OS_TASK_STORAGE<F>,
}

/*
****************************************************************************************************************************************
*                                                             implement of structure
****************************************************************************************************************************************
*/

impl OS_TCB {
    // can only be called if the task owns the stack
    pub(crate) fn restore_context_from_stk(&mut self) {
        extern "Rust" {
            fn restore_thread_task();
        }
        if self.OSTCBStkPtr.is_none() {
            return;
        }
        // in restore_task it will set PROGRAM_STACK a new stk
        // revoke the stk
        critical_section::with(|_| unsafe { restore_thread_task() });
    }
    /// get the stk ptr of tcb, and set the tcb's stk ptr to None
    pub fn take_stk(&mut self) -> OS_STK_REF {
        self.OSTCBStkPtr.take().unwrap()
    }
    /// set the stk ptr of tcb
    pub fn set_stk(&mut self, stk: OS_STK_REF) {
        self.OSTCBStkPtr = Some(stk);
    }
    /// by noah: *TEST*, judge whether the task stk is none
    pub fn is_stk_none(&self) -> bool {
        self.OSTCBStkPtr.is_none()
    }
}

#[cfg(feature = "OS_TASK_CREATE_EXT_EN")]
impl OS_TCB_EXT {
    fn init(&mut self, pext: *mut (), opt: INT16U, id: INT16U) {
        self.OSTCBExtPtr = pext;
        // info about stack is no need to be init here
        // self.OSTCBStkBottom=None;
        // self.OSTCBStkSize=0;
        self.OSTCBOpt = opt;
        self.OSTCBId = id;
    }
}

impl<F: Future + 'static> OS_TASK_STORAGE<F> {
    /// create a new OS_TASK_STORAGE
    // Take a lazy approach, which means the TCB will be init when call the init func of TCB
    // this func will be used to init the global array
    const fn new() -> Self {
        Self {
            task_tcb: OS_TCB {
                OSTCBStkPtr: None,
                #[cfg(feature = "OS_TASK_CREATE_EXT_EN")]
                OSTCBExtInfo: OS_TCB_EXT {
                    OSTCBExtPtr: 0 as PTR,
                    OSTCBStkBottom: None,
                    OSTCBStkSize: 0,
                    OSTCBOpt: 0,
                    OSTCBId: 0,
                },
                OSTimerNext: SyncUnsafeCell::new(None),
                OSTimerPrev: SyncUnsafeCell::new(None),
                OS_POLL_FN: SyncUnsafeCell::new(None),
                #[cfg(feature = "OS_EVENT_EN")]
                OSTCBEventPtr: None,
                #[cfg(any(all(feature = "OS_Q_EN", feature = "OS_MAX_QS"), feature = "OS_MBOX_EN"))]
                OSTCBMsg: 0 as PTR,
                OSTCBDly: 0,
                OSTCBStat: State::new(),
                // no need
                // OSTCBStatPend: 0,
                OSTCBPrio: 0,
                OSTCBX: 0,
                OSTCBY: 0,
                OSTCBBitX: 0,
                OSTCBBitY: 0,
                #[cfg(feature = "OS_TASK_DEL_EN")]
                OSTCBDelReq: 0,

                #[cfg(feature = "OS_TASK_PROFILE_EN")]
                OSTCBCtxSwCtr: 0,
                #[cfg(feature = "OS_TASK_PROFILE_EN")]
                OSTCBCyclesTot: 0,
                #[cfg(feature = "OS_TASK_PROFILE_EN")]
                OSTCBCyclesStart: 0,
                #[cfg(feature = "OS_TASK_PROFILE_EN")]
                OSTCBStkBase: None,
                #[cfg(feature = "OS_TASK_PROFILE_EN")]
                OSTCBStkUsed: 0,
                
                #[cfg(feature = "OS_TASK_REG_TBL_SIZE")]
                OSTCBRegTbl: [0; OS_TASK_REG_TBL_SIZE],
                #[cfg(feature = "OS_TASK_NAME_EN")]
                OSTCBTaskName: String::new(),
                expires_at: SyncUnsafeCell::new(u64::MAX),
                is_in_thread_poll: SyncUnsafeCell::new(true),
            },
            future: UninitCell::uninit(),
        }
    }

    /// init the storage of the task, just like the spawn in Embassy
    //  this func will be called by OS_TASK_CTREATE
    //  just like OSTCBInit in uC/OS, but we don't need the stack ptr
    pub fn init(
        prio: INT8U,
        id: INT16U,
        pext: *mut (),
        opt: INT16U,
        _name: String,
        future_func: impl FnOnce() -> F,
    ) -> OS_ERR_STATE {
        #[cfg(feature = "defmt")]
        trace!("init of OS_TASK_STORAGE");
        // by noah: claim a TaskStorage
        let task_ref = OS_TASK_STORAGE::<F>::claim();

        let this: &mut OS_TASK_STORAGE<F>;
        unsafe {
            this = &mut *(task_ref.as_ptr() as *mut OS_TASK_STORAGE<F>);
            this.task_tcb.OS_POLL_FN.set(Some(OS_TASK_STORAGE::<F>::poll));
            this.future.write_in_place(future_func);
        }
        // set the prio also need to set it in the bitmap
        this.task_tcb.OSTCBPrio = prio;
        this.task_tcb.OSTCBY = prio >> 3;
        this.task_tcb.OSTCBX = prio & 0x07;
        this.task_tcb.OSTCBBitY = 1 << this.task_tcb.OSTCBY;
        this.task_tcb.OSTCBBitX = 1 << this.task_tcb.OSTCBX;
        // set the stat
        if !this.task_tcb.OSTCBStat.spawn() {
            panic!("task with prio {} spawn failed", prio);
        }
        // init ext info
        #[cfg(feature = "OS_TASK_CREATE_EXT_EN")]
        this.task_tcb.OSTCBExtInfo.init(pext, opt, id);
        // add the task to ready queue
        // the operation about the bitmap will be done in the RunQueue
        // need a cs
        critical_section::with(|_cs| {
            unsafe { GlobalSyncExecutor.as_ref().unwrap().enqueue(task_ref) };
        });
        #[cfg(feature = "OS_EVENT_EN")]
        {
            this.task_tcb.OSTCBEventPtr = None;
            #[cfg(feature = "OS_EVENT_MULTI_EN")]
            {
                // this.task_tcb.OSTCBEventMultiPtr
                // this.task_tcb.OSTCBEventMultiPtr
            }
        }
        #[cfg(any(feature = "OS_MBOX_EN", all(feature = "OS_Q_EN", feature = "OS_MAX_QS")))]
        {
            this.task_tcb.OSTCBMsg = 0 as PTR;
        }
        #[cfg(feature = "OS_TASK_NAME_EN")]
        {
            this.task_tcb.OSTCBTaskName = name;
        }

        #[cfg(feature = "OS_TASK_REG_TBL_SIZE")]
        if OS_TASK_REG_TBL_SIZE > 0 {
            for i in 0..OS_TASK_REG_TBL_SIZE {
                this.task_tcb.OSTCBRegTbl[i] = 0;
            }
        }

        #[cfg(feature = "OS_CPU_HOOKS_EN")]
        {
            // Call user defined hook
            OSTCBInitHook(ptcb);
            OSTaskCreateHook(ptcb);
        }
        return OS_ERR_STATE::OS_ERR_NONE;
        // we don't need to add the TaskRef into OSTCBPrioTbl because we did this in func enqueue
    }

    /// the poll function will be called uniquely once by the executor
    unsafe fn poll(p: OS_TCB_REF) {
        let this = &*(p.as_ptr() as *const OS_TASK_STORAGE<F>);

        let future = Pin::new_unchecked(this.future.as_mut());
        let waker = waker::from_task(p);
        let mut cx = Context::from_waker(&waker);
        match future.poll(&mut cx) {
            Poll::Ready(_) => {
                #[cfg(feature = "defmt")]
                trace!("the task {} is ready", this.task_tcb.OSTCBPrio);
                this.future.drop_in_place();
                this.task_tcb.OSTCBStat.despawn();
            }
            Poll::Pending => {
                #[cfg(feature = "defmt")]
                trace!("the task {} is pending", this.task_tcb.OSTCBPrio);
            }
        }

        // the compiler is emitting a virtual call for waker drop, but we know
        // it's a noop for our waker.
        mem::forget(waker);
    }

    /// this func will be called to create a new task(TCB)
    // refer to the get of TaskPoolRef in embassy
    fn claim() -> OS_TCB_REF {
        // by noah: for we can create task after OSTaskCreate, so we need a cs
        critical_section::with(|cs| {
            let task_storage = ARENA.alloc::<OS_TASK_STORAGE<F>>(cs);
            #[cfg(feature = "alarm_test")]
            trace!("size of the task storage is {}", mem::size_of::<OS_TASK_STORAGE<F>>());
            // create a new task which is not init
            task_storage.write(OS_TASK_STORAGE::new());
            // by noah：no panic will occurred here because if the Arena is not enough, the program will panic when alloc
            OS_TCB_REF {
                ptr: Some(NonNull::new(task_storage as *mut _ as _).unwrap()),
            }
        })
    }
}

unsafe impl Sync for OS_TCB_REF {}
unsafe impl Send for OS_TCB_REF {}

impl Default for OS_TCB_REF {
    // this func will not be called
    fn default() -> Self {
        // by noah:dangling is used to create a dangling pointer, which is just like the null pointer in C
        OS_TCB_REF { ptr: None }
    }
}

// impl deref for OS_TCB_REF
impl Deref for OS_TCB_REF {
    type Target = OS_TCB;
    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.unwrap().as_ref() }
    }
}

// impl deref for mut OS_TCB_REF
impl DerefMut for OS_TCB_REF {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.ptr.unwrap().as_mut() }
    }
}

impl OS_TCB_REF {
    /// Safety: The pointer must have been obtained with `Task::as_ptr`
    pub(crate) unsafe fn from_ptr(ptr: *const OS_TCB) -> Self {
        Self {
            ptr: Some(NonNull::new_unchecked(ptr as *mut OS_TCB)),
        }
    }

    pub(crate) fn header(self) -> &'static OS_TCB {
        unsafe { self.ptr.unwrap().as_ref() }
    }

    /// The returned pointer is valid for the entire OS_TASK_STORAGE.
    pub(crate) fn as_ptr(self) -> *const OS_TCB {
        self.ptr.unwrap().as_ptr()
    }
}

impl<F: Future + 'static> AvailableTask<F> {}
