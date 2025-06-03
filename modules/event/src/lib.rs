#![no_std]
#![no_main]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

//! event

/// the mod of flag of uC/OS-II kernel
pub mod os_flag;
/// the mod of semaphore of uC/OS-II kernel
pub mod os_sem;
/// the mod of mailbox of uC/OS-II kernel
pub mod os_mbox;
/// the mod of mutex of uC/OS-II kernel
pub mod os_mutex;
/// the mod of queue of uC/OS-II kernel
pub mod os_q;

use core::ptr::NonNull;
#[cfg(feature = "OS_EVENT_NAME_EN")]
use alloc::string::String;
use core::ops::{Deref, DerefMut};

use lazy_static::lazy_static;
use critical_section::{self, CriticalSection};

use cfg::{INT16U, OS_MAX_EVENTS, OS_LOWEST_PRIO};
use cfg::ucosii::{OS_PRIO, OS_EVENT_TBL_SIZE};
use cell::SyncUnsafeCell;
use executor::{GlobalSyncExecutor, OSUnMapTbl};
use executor::{task::OS_TCB_REF, mem::arena::ARENA};


/*
*********************************************************************************************************
*                                         EVENT CONTROL BLOCK
*********************************************************************************************************
*/

// #[cfg(feature = "OS_EVENT_EN")]
#[repr(C)]
#[allow(unused)]
#[allow(missing_docs)]
/// the event control block
pub struct OS_EVENT {
    pub OSEventType: OS_EVENT_TYPE,         /* Type of event control block (see OS_EVENT_TYPE_xxxx)    */
    pub OSEventPtr: SyncUnsafeCell<Option<OS_EVENT_REF>>, /* Pointer to message or queue structure                   */
    pub OSEventCnt: INT16U,         /* Semaphore Count (not used if other EVENT type)          */
    pub OSEventGrp: OS_PRIO,        /* Group corresponding to tasks waiting for event to occur */
    pub OSEventTbl: [OS_PRIO; OS_EVENT_TBL_SIZE as usize], /* List of tasks waiting for event to occur                */
    #[cfg(feature = "OS_EVENT_NAME_EN")]
    pub OSEventName: String, // the name of the event
}

/// the ref of ECB
// #[cfg(feature = "OS_EVENT_EN")]
#[allow(unused)]
#[derive(Clone, Copy)]
pub struct OS_EVENT_REF {
    /// the pointer to the ecb
    pub ptr: Option<NonNull<OS_EVENT>>,
}

/// the type of event
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OS_EVENT_TYPE {
    /// the unused type
    UNUSED = 0,
    /// the mailbox type
    MBOX = 1,
    /// the queue type
    Q = 2,
    /// the semaphore type
    SEM = 3,
    /// the mutex type
    MUTEX = 4,
    /// the flag type
    FLAG = 5,
}

impl OS_EVENT {
    /// create a new event
    pub fn new() -> Self {
        OS_EVENT {
            OSEventType: OS_EVENT_TYPE::UNUSED,
            OSEventPtr: SyncUnsafeCell::new(None),
            OSEventCnt: 0,
            OSEventGrp: 0,
            OSEventTbl: [0; OS_EVENT_TBL_SIZE as usize],
            #[cfg(feature = "OS_EVENT_NAME_EN")]
            OSEventName: String::new(),
        }
    }

    /// initialize the event wait list
    pub fn OS_EventWaitListInit(&mut self) {
        //  No task waiting on event
        self.OSEventGrp = 0;
        // init the event wait list
        for i in 0..OS_EVENT_TBL_SIZE {
            self.OSEventTbl[i] = 0;
        }
    }
}

#[allow(unused)]
impl OS_EVENT_REF {
    /// The pointer must have been obtained with OS_EVENT
    pub(crate) unsafe fn from_ptr(ptr: *const OS_EVENT) -> Self {
        Self {
            ptr: Some(NonNull::new_unchecked(ptr as *mut OS_EVENT)),
        }
    }
    pub(crate) fn header(self) -> &'static OS_EVENT {
        unsafe { self.ptr.unwrap().as_ref() }
    }
    pub(crate) fn as_ptr(self) -> *const OS_EVENT {
        self.ptr.unwrap().as_ptr()
    }
}

unsafe impl Sync for OS_EVENT_REF {}
unsafe impl Send for OS_EVENT_REF {}

impl Default for OS_EVENT_REF {
    // this func will not be called
    fn default() -> Self {
        // dangling is used to create a dangling pointer, which is just like the null pointer in C
        OS_EVENT_REF { ptr: None }
    }
}

// impl deref for OS_TCB_REF
impl Deref for OS_EVENT_REF {
    type Target = OS_EVENT;
    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.unwrap().as_ref() }
    }
}

// impl deref for mut OS_TCB_REF
impl DerefMut for OS_EVENT_REF {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.ptr.unwrap().as_mut() }
    }
}

/*
*********************************************************************************************************
*                                         EVENT POOL
*********************************************************************************************************
*/

lazy_static! {
    /// the global event pool
    pub static ref GlobalEventPool: Option<EventPool> = Some(EventPool::new());
}

/// the event pool
pub struct EventPool {
    /// Pointer to free list of event control blocks
    pub OSEventFreeList: SyncUnsafeCell<Option<OS_EVENT_REF>>, 
    OSEventTbl:  SyncUnsafeCell<[OS_EVENT_REF; OS_MAX_EVENTS as usize]>,
}

impl EventPool {
    /// the event pool
    pub fn new() -> Self {
        EventPool {
            OSEventFreeList: SyncUnsafeCell::new(None),
            OSEventTbl: SyncUnsafeCell::new([OS_EVENT_REF::default(); OS_MAX_EVENTS as usize]),
        }
    }
    /// init the event table
    pub unsafe fn init(&self) {
        critical_section::with(|cs| {
            for i in 0..OS_MAX_EVENTS {
                if self.OSEventTbl.get_mut()[i].ptr.is_none() {
                    self.OSEventTbl.get_mut()[i] = EventPool::claim(cs);
                }
            }
        });
        let mut pevent1: OS_EVENT_REF;
        let mut pevent2: OS_EVENT_REF;
        for i in 0..OS_MAX_EVENTS-1 {
            pevent1 = self.OSEventTbl.get_mut()[i];
            pevent2 = self.OSEventTbl.get_mut()[i+1];
            pevent1.OSEventType = OS_EVENT_TYPE::UNUSED;
            pevent1.OSEventPtr.set(Some(pevent2));   
        }
        pevent1 = self.OSEventTbl.get_mut()[OS_MAX_EVENTS-1];
        pevent1.OSEventType = OS_EVENT_TYPE::UNUSED;
        pevent1.OSEventPtr.set(None);

        self.OSEventFreeList.set(Some(self.OSEventTbl.get_mut()[0]));
    }
    /// alloc a new event from the event pool
    pub fn alloc(&self) -> Option<OS_EVENT_REF> {
        critical_section::with(|cs| {
            let free_list = self.OSEventFreeList.get_mut();
            // there are no free ECB in EventTbl
            if free_list.is_none() {
                return None;
            }
            let mut event_ref = free_list.unwrap();
            // if the free list is empty, then we need to allocate a new event
            if event_ref.ptr.is_none() && event_ref.OSEventType == OS_EVENT_TYPE::UNUSED 
            {
                event_ref = EventPool::claim(cs);
            } 
            else if event_ref.ptr.is_some() && event_ref.OSEventType == OS_EVENT_TYPE::UNUSED 
            {
                // this event has been free before 
                event_ref.OSEventCnt = 0;
                event_ref.OSEventGrp = 0;
                event_ref.OSEventTbl = [0; OS_EVENT_TBL_SIZE as usize];
                #[cfg(feature = "OS_EVENT_NAME_EN")]
                {
                    event_ref.OSEventName = String::from("?");
                }
            } 
            else 
            {
                // this is error
                return None;
            }
            unsafe {
                self.OSEventFreeList.set(event_ref.OSEventPtr.get());
            }   
            return Some(event_ref);
        })
    }
    /// free the event
    pub fn free(&self, mut event: OS_EVENT_REF) {
        critical_section::with(|_| {
            event.OSEventType = OS_EVENT_TYPE::UNUSED;
            unsafe {
                event.OSEventPtr.set(self.OSEventFreeList.get());
                self.OSEventFreeList.set(Some(event));
            }
        })
    } 
    /// this func will be called to create a new event
    fn claim(cs: CriticalSection) -> OS_EVENT_REF {
        let event = ARENA.alloc::<OS_EVENT>(cs);
        event.write(OS_EVENT::new());
        
        OS_EVENT_REF { 
            ptr: Some(NonNull::new(event as *mut _ as _).unwrap()) 
        }
    }
}

// #[cfg(feature = "OS_EVENT_EN")]
/// used to ready a task that was waiting for an event to occur
pub fn OS_EventTaskRdy(pevent: OS_EVENT_REF) {
    let mut prio: u8 = 0;
    if OS_LOWEST_PRIO <= 63 {
        // find HPT waiting for message
        let y = OSUnMapTbl[pevent.OSEventGrp as usize];
        let x = OSUnMapTbl[pevent.OSEventTbl[y as usize] as usize];
        // find priority of task getting the msg
        prio = (y << 3) + x;
    }

    let executor = GlobalSyncExecutor.as_ref().unwrap();
    let prio_tbl = executor.get_prio_tbl();
    let ptcb = prio_tbl[prio as usize];

    unsafe{ 
        ptcb.expires_at.set(u64::MAX); 
        // put task in the ready to run list
        executor.enqueue(ptcb); 
    }
    // remove this task from event wait list
    OS_EventTaskRemove(ptcb, pevent);
}

// #[cfg(feature = "OS_EVENT_EN")]
/// suspend a task because an event has not occurred
pub fn OS_EventTaskWait(mut pevent: OS_EVENT_REF) {
    let executor = GlobalSyncExecutor.as_ref().unwrap();
    let task = executor.OSTCBCur.get_unmut();
    unsafe {
        #[cfg(feature = "OS_EVENT_EN")]
        // store ptr to ECB in TCB
        task.OSTCBEventPtr.set(Some(pevent));
        // task no longer ready
        executor.set_task_unready(*task);
    }
    // put task in waiting list
    pevent.OSEventTbl[task.OSTCBY as usize] |= task.OSTCBX;
    pevent.OSEventGrp |= task.OSTCBY;
}

// #[cfg(feature = "OS_EVENT_EN")]
/// remove a task from an event's wait list
pub fn OS_EventTaskRemove(ptcb: OS_TCB_REF, mut pevent: OS_EVENT_REF) {
    let y = ptcb.OSTCBY;
    // remove task from wait list
    pevent.OSEventTbl[y as usize] &= !ptcb.OSTCBX;
    if pevent.OSEventTbl[y as usize] == 0 {
        pevent.OSEventGrp &= !ptcb.OSTCBY;
    }
    unsafe {
        #[cfg(feature = "OS_EVENT_EN")]
        ptcb.OSTCBEventPtr.set(None);
    }
}