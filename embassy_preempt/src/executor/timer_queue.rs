//! Timer Queue for task delay
#[cfg(feature = "defmt")]
#[allow(unused)]
use defmt::{trace,info};

use super::OS_TCB_REF;
use crate::util::SyncUnsafeCell;

pub(crate) struct TimerQueue {
    head: SyncUnsafeCell<Option<OS_TCB_REF>>,   // head of the timer queue, indicating the earliest arriving task
    pub(crate) set_time: SyncUnsafeCell<u64>,   // timestamp of the earliest arriving task
}

impl TimerQueue {
    pub const fn new() -> Self {
        Self {
            head: SyncUnsafeCell::new(None),
            set_time: SyncUnsafeCell::new(u64::MAX),
        }
    }

    /// Insert a task into the timer queue.(sorted by `expires_at`,the header is the nearest task)
    /// return the next expiration time.
    pub(crate) unsafe fn update(&self, p: OS_TCB_REF) -> u64 {
        // #[cfg(feature = "defmt")]
        // trace!("in timer update");
        let p_expires_at = &p.expires_at;
        // by noah：this indicate that the time queue is not updated or the time queue is null
        if *p_expires_at.get_unmut() == u64::MAX {
            return u64::MAX;
        }
        // range from head to find one larger than p_expires_at and insert p.
        let mut cur = self.head.get();
        let mut prev: Option<OS_TCB_REF> = None;
        while let Some(cur_ref) = cur {
            let cur_expires_at = &cur_ref.expires_at;
            if cur_expires_at > p_expires_at {
                break;
            }
            prev = cur;
            cur = cur_ref.OSTimerNext.get();
        }
        // insert p
        p.OSTimerNext.set(cur);
        p.OSTimerPrev.set(prev);
        if let Some(cur_ref) = cur {
            cur_ref.OSTimerPrev.set(Some(p));
        }
        if let Some(prev_ref) = prev {
            prev_ref.OSTimerNext.set(Some(p));
        } else {
            self.head.set(Some(p));
        }
        // #[cfg(feature = "defmt")]
        // trace!("exit timer update");
        return *self.head.get_unmut().as_ref().unwrap().expires_at.get_unmut();
    }

    /// get the arrival time of the earliest arriving task
    pub(crate) unsafe fn next_expiration(&self) -> u64 {
        let head = self.head.get_unmut();
        if let Some(head_ref) = head {
            *head_ref.expires_at.get_unmut()
        } else {
            u64::MAX
        }
    }
    
    /// wake up all tasks whose delay time has arrived
    pub(crate) unsafe fn dequeue_expired(&self, now: u64, on_task: impl Fn(OS_TCB_REF)) {
        #[cfg(feature = "defmt")]
        trace!("dequeue expired");
        let mut cur = self.head.get();
        while let Some(cur_ref) = cur {
            let cur_expires_at = &cur_ref.expires_at;
            if *cur_expires_at.get_unmut() > now {
                break;
            }
            on_task(cur_ref);
            // by noah: clear the expire time
            cur_ref.expires_at.set(u64::MAX);
            let next = cur_ref.OSTimerNext.get();
            let prev = cur_ref.OSTimerPrev.get();
            if let Some(next_ref) = next {
                next_ref.OSTimerPrev.set(prev);
            }
            if let Some(prev_ref) = prev {
                prev_ref.OSTimerNext.set(next);
            } else {
                self.head.set(next);
            }
            // unset the cur's next and prev: fix by liam
            cur_ref.OSTimerNext.set(None);
            cur_ref.OSTimerPrev.set(None);
            cur = next;
        }
    }

    /// remove a task from the timer queue
    pub(crate) unsafe fn remove(&self, p: OS_TCB_REF) {
        if self.head.get().is_none() {
            return;
        }
        let mut cur = self.head.get();
        let mut found = false;

        // traverse the queue to find the target node
        while let Some(cur_ref) = cur {
            if cur_ref == p {
                found = true;
                break;
            }
            cur = cur_ref.OSTimerNext.get();
        }
        if !found {
            return;
        }
        // remove the target node from the timer queue
        let prev = p.OSTimerPrev.get();
        let next = p.OSTimerNext.get();

        p.OSTimerNext.set(None);
        p.OSTimerPrev.set(None);
        // update the `OSTimerNext` pointer of the previous node
        if let Some(prev_ref) = prev {
            prev_ref.OSTimerNext.set(next);
        } else {
            self.head.set(next);
        }
        // update the `OSTimerPrev` pointer of the back node
        if let Some(next_ref) = next {
            next_ref.OSTimerPrev.set(prev);
        }
    }
}
