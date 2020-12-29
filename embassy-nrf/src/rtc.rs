use core::cell::Cell;
use core::ops::Deref;
use core::sync::atomic::{AtomicU32, Ordering};

use embassy::time::{Clock, Instant};

use crate::interrupt;
use crate::interrupt::{CriticalSection, Mutex, OwnedInterrupt};
use crate::pac::{rtc0, Interrupt, RTC0, RTC1};

#[cfg(any(feature = "52832", feature = "52833", feature = "52840"))]
use crate::pac::RTC2;

fn calc_now(period: u32, counter: u32) -> u64 {
    let shift = ((period & 1) << 23) + 0x400000;
    let counter_shifted = (counter + shift) & 0xFFFFFF;
    ((period as u64) << 23) + counter_shifted as u64 - 0x400000
}

fn compare_n(n: usize) -> u32 {
    1 << (n + 16)
}

mod test {
    use super::*;

    #[test]
    fn test_calc_now() {
        assert_eq!(calc_now(0, 0x000000), 0x0_000000);
        assert_eq!(calc_now(0, 0x000001), 0x0_000001);
        assert_eq!(calc_now(0, 0x7FFFFF), 0x0_7FFFFF);
        assert_eq!(calc_now(1, 0x7FFFFF), 0x0_7FFFFF);
        assert_eq!(calc_now(0, 0x800000), 0x0_800000);
        assert_eq!(calc_now(1, 0x800000), 0x0_800000);
        assert_eq!(calc_now(1, 0x800001), 0x0_800001);
        assert_eq!(calc_now(1, 0xFFFFFF), 0x0_FFFFFF);
        assert_eq!(calc_now(2, 0xFFFFFF), 0x0_FFFFFF);
        assert_eq!(calc_now(1, 0x000000), 0x1_000000);
        assert_eq!(calc_now(2, 0x000000), 0x1_000000);
    }
}

struct AlarmState {
    timestamp: Cell<u64>,
    callback: Cell<Option<fn()>>,
}

impl AlarmState {
    fn new() -> Self {
        Self {
            timestamp: Cell::new(u64::MAX),
            callback: Cell::new(None),
        }
    }
}

const ALARM_COUNT: usize = 3;

pub struct RTC<T: Instance> {
    rtc: T,
    irq: T::Interrupt,

    /// Number of 2^23 periods elapsed since boot.
    ///
    /// This is incremented by 1
    /// - on overflow (counter value 0)
    /// - on "midway" between overflows (at counter value 0x800000)
    ///
    /// Therefore: When even, counter is in 0..0x7fffff. When odd, counter is in 0x800000..0xFFFFFF
    /// This allows for now() to return the correct value even if it races an overflow.
    ///
    /// It overflows on 2^32 * 2^23 / 32768 seconds of uptime, which is 34865 years.
    period: AtomicU32,

    /// Timestamp at which to fire alarm. u64::MAX if no alarm is scheduled.
    alarms: Mutex<[AlarmState; ALARM_COUNT]>,
}

unsafe impl<T: Instance> Send for RTC<T> {}
unsafe impl<T: Instance> Sync for RTC<T> {}

impl<T: Instance> RTC<T> {
    pub fn new(rtc: T, irq: T::Interrupt) -> Self {
        Self {
            rtc,
            irq,
            period: AtomicU32::new(0),
            alarms: Mutex::new([AlarmState::new(), AlarmState::new(), AlarmState::new()]),
        }
    }

    pub fn start(&'static self) {
        self.rtc.cc[3].write(|w| unsafe { w.bits(0x800000) });

        self.rtc.intenset.write(|w| {
            let w = w.ovrflw().set();
            let w = w.compare3().set();
            w
        });

        self.rtc.tasks_clear.write(|w| unsafe { w.bits(1) });
        self.rtc.tasks_start.write(|w| unsafe { w.bits(1) });

        // Wait for clear
        while self.rtc.counter.read().bits() != 0 {}

        T::set_rtc_instance(self);
        self.irq
            .set_handler(|| T::get_rtc_instance().on_interrupt());
        self.irq.unpend();
        self.irq.enable();
    }

    fn on_interrupt(&self) {
        if self.rtc.events_ovrflw.read().bits() == 1 {
            self.rtc.events_ovrflw.write(|w| w);
            self.next_period();
        }

        if self.rtc.events_compare[3].read().bits() == 1 {
            self.rtc.events_compare[3].write(|w| w);
            self.next_period();
        }

        for n in 0..ALARM_COUNT {
            if self.rtc.events_compare[n].read().bits() == 1 {
                self.rtc.events_compare[n].write(|w| w);
                interrupt::free(|cs| {
                    self.trigger_alarm(n, cs);
                })
            }
        }
    }

    fn next_period(&self) {
        interrupt::free(|cs| {
            let period = self.period.fetch_add(1, Ordering::Relaxed) + 1;
            let t = (period as u64) << 23;

            for n in 0..ALARM_COUNT {
                let alarm = &self.alarms.borrow(cs)[n];
                let at = alarm.timestamp.get();

                let diff = at - t;
                if diff < 0xc00000 {
                    self.rtc.cc[n].write(|w| unsafe { w.bits(at as u32 & 0xFFFFFF) });
                    self.rtc.intenset.write(|w| unsafe { w.bits(compare_n(n)) });
                }
            }
        })
    }

    fn trigger_alarm(&self, n: usize, cs: &CriticalSection) {
        self.rtc.intenclr.write(|w| unsafe { w.bits(compare_n(n)) });

        let alarm = &self.alarms.borrow(cs)[n];
        alarm.timestamp.set(u64::MAX);

        // Call after clearing alarm, so the callback can set another alarm.
        alarm.callback.get().map(|f| f());
    }

    fn set_alarm_callback(&self, n: usize, callback: fn()) {
        interrupt::free(|cs| {
            let alarm = &self.alarms.borrow(cs)[n];
            alarm.callback.set(Some(callback));
        })
    }

    fn set_alarm(&self, n: usize, timestamp: u64) {
        interrupt::free(|cs| {
            let alarm = &self.alarms.borrow(cs)[n];
            alarm.timestamp.set(timestamp);

            let t = self.now();
            if timestamp <= t {
                self.trigger_alarm(n, cs);
                return;
            }

            let diff = timestamp - t;
            if diff < 0xc00000 {
                self.rtc.cc[n].write(|w| unsafe { w.bits(timestamp as u32 & 0xFFFFFF) });
                self.rtc.intenset.write(|w| unsafe { w.bits(compare_n(n)) });

                // We may have been preempted for arbitrary time between checking if `at` is in the past
                // and setting the cc. In that case, we don't know if the cc has triggered.
                // So, we check again just in case.

                let t = self.now();
                if timestamp <= t {
                    self.trigger_alarm(n, cs);
                    return;
                }
            } else {
                self.rtc.intenclr.write(|w| unsafe { w.bits(compare_n(n)) });
            }
        })
    }

    pub fn alarm0(&'static self) -> Alarm<T> {
        Alarm { n: 0, rtc: self }
    }
    pub fn alarm1(&'static self) -> Alarm<T> {
        Alarm { n: 1, rtc: self }
    }
    pub fn alarm2(&'static self) -> Alarm<T> {
        Alarm { n: 2, rtc: self }
    }
}

impl<T: Instance> embassy::time::Clock for RTC<T> {
    fn now(&self) -> u64 {
        let counter = self.rtc.counter.read().bits();
        let period = self.period.load(Ordering::Relaxed);
        calc_now(period, counter)
    }
}

pub struct Alarm<T: Instance> {
    n: usize,
    rtc: &'static RTC<T>,
}

impl<T: Instance> embassy::time::Alarm for Alarm<T> {
    fn set_callback(&self, callback: fn()) {
        self.rtc.set_alarm_callback(self.n, callback);
    }

    fn set(&self, timestamp: u64) {
        self.rtc.set_alarm(self.n, timestamp);
    }

    fn clear(&self) {
        self.rtc.set_alarm(self.n, u64::MAX);
    }
}

/// Implemented by all RTC instances.
pub trait Instance: Deref<Target = rtc0::RegisterBlock> + Sized + 'static {
    /// The interrupt associated with this RTC instance.
    type Interrupt: OwnedInterrupt;

    fn set_rtc_instance(rtc: &'static RTC<Self>);
    fn get_rtc_instance() -> &'static RTC<Self>;
}

macro_rules! impl_instance {
    ($name:ident, $irq_name:path, $static_name:ident) => {
        static mut $static_name: Option<&'static RTC<$name>> = None;

        impl Instance for $name {
            type Interrupt = $irq_name;
            fn set_rtc_instance(rtc: &'static RTC<Self>) {
                unsafe { $static_name = Some(rtc) }
            }
            fn get_rtc_instance() -> &'static RTC<Self> {
                unsafe { $static_name.unwrap() }
            }
        }
    };
}

impl_instance!(RTC0, interrupt::RTC0Interrupt, RTC0_INSTANCE);
impl_instance!(RTC1, interrupt::RTC1Interrupt, RTC1_INSTANCE);

#[cfg(any(feature = "52832", feature = "52833", feature = "52840"))]
impl_instance!(RTC2, interrupt::RTC2Interrupt, RTC2_INSTANCE);
