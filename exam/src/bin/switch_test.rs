#![no_main]
#![no_std]
#![feature(impl_trait_in_assoc_type)]

// FFI接口
use core::ffi::c_void;
use core::arch::asm;

use executor::{SyncOSTaskCreate, AsyncOSTaskCreate};
use executor::os_core::{OSInit, OSStart};
use executor::os_time::OSTimeDly;
use executor::pac::{gpio, GPIOA, RCC};
// use app::led::Pin_Init;

#[cfg(feature = "defmt")]
use defmt::info;
#[cfg(feature = "alarm_test")]
use defmt::info;

#[cortex_m_rt::entry]
fn usart_test() -> ! {
    #[cfg(feature = "alarm_test")]
    info!("OS Start");

    led_init();
    // Pin_Init();
    pin_init();
    // usart_init();

    OSInit();
    
    AsyncOSTaskCreate(task1, 0 as *mut c_void, 0 as *mut usize, 10);
    // AsyncOSTaskCreate(task2, 0 as *mut c_void, 0 as *mut usize, 11);
    AsyncOSTaskCreate(task3, 0 as *mut c_void, 0 as *mut usize, 12);
    AsyncOSTaskCreate(task4, 0 as *mut c_void, 0 as *mut usize, 13);
    AsyncOSTaskCreate(task5, 0 as *mut c_void, 0 as *mut usize, 14);

    OSStart();
}

// async fn task1(_args: *mut c_void) {
//     loop {
//         #[cfg(feature = "defmt")]
//         info!("usart_test");
//     //    usart_send_byte(b'A');
//        OSTimeDly(400 * 100);
//     }
// }

async fn task1(_args: *mut c_void) {
    loop {
        led_on();
        OSTimeDly(500 * 100);
        led_off();
        OSTimeDly(500 * 100);
    }
}

async fn task2(_args: *mut c_void) {
    loop {
        // critical_section::with(|_| task_pin_low(2));
        task_pin_low(2);
        delay(1);
        #[cfg(feature = "alarm_test")]
        trace!("the task2");
        delay(1);
        // critical_section::with(|_| task_pin_high(2));
        task_pin_high(2);
        OSTimeDly(50 * 100);
    }
}

async fn task3(_args: *mut c_void) {
    loop {
        task_pin_low(3);
        delay(1);
        delay(1);
        task_pin_high(3);
        #[cfg(feature = "alarm_test")]
        trace!("the task3");
        OSTimeDly(500 * 100);
    }
}

async fn task4(_args: *mut c_void) {
    loop {
        task_pin_low(4);
        delay(1);
        #[cfg(feature = "alarm_test")]
        trace!("the task4");
        delay(1);
        task_pin_high(4);
        OSTimeDly(50 * 100);
    }
}

async fn task5(_args: *mut c_void) {
    loop {
        task_pin_low(5);
        delay(1);
        #[cfg(feature = "alarm_test")]
        trace!("the task5");
        delay(1);
        task_pin_high(5);
        OSTimeDly(50 * 100);
    }
}

#[allow(dead_code)]
pub fn led_init() {
    RCC.ahb1enr().modify(|f| {
        f.set_gpioaen(true);
    });
    GPIOA.moder().modify(|f| {
        f.set_moder(5, gpio::vals::Moder::OUTPUT);
    });
    GPIOA.otyper().modify(|f| {
        f.set_ot(5, gpio::vals::Ot::PUSHPULL);
    });
    GPIOA.ospeedr().modify(|f| {
        f.set_ospeedr(5, gpio::vals::Ospeedr::HIGHSPEED);
    });
    GPIOA.pupdr().modify(|v| {
        v.set_pupdr(5, gpio::vals::Pupdr::FLOATING);
    });
    GPIOA.odr().modify(|v| {
        v.set_odr(5, gpio::vals::Odr::HIGH);
    });
}

#[allow(dead_code)]
pub fn pin_init() {
    // enable the RCC
    RCC.ahb1enr().modify(|v| {
        v.set_gpioaen(true);
    });
    // set GPIO
    GPIOA.moder().modify(|v| {
        // set mode as output
        v.set_moder(0, gpio::vals::Moder::OUTPUT);
        v.set_moder(1, gpio::vals::Moder::OUTPUT);
        v.set_moder(4, gpio::vals::Moder::OUTPUT);
        v.set_moder(6, gpio::vals::Moder::OUTPUT);
        v.set_moder(7, gpio::vals::Moder::OUTPUT);
        v.set_moder(8, gpio::vals::Moder::OUTPUT);
    });
    GPIOA.otyper().modify(|v| {
        // set output type as push-pull
        v.set_ot(0, gpio::vals::Ot::PUSHPULL);
        v.set_ot(1, gpio::vals::Ot::PUSHPULL);
        v.set_ot(4, gpio::vals::Ot::PUSHPULL);
        v.set_ot(6, gpio::vals::Ot::PUSHPULL);
        v.set_ot(7, gpio::vals::Ot::PUSHPULL);
        v.set_ot(8, gpio::vals::Ot::PUSHPULL);
    });
    GPIOA.ospeedr().modify(|v| {
        // set output speed as high
        v.set_ospeedr(0, gpio::vals::Ospeedr::HIGHSPEED);
        v.set_ospeedr(1, gpio::vals::Ospeedr::HIGHSPEED);
        v.set_ospeedr(4, gpio::vals::Ospeedr::HIGHSPEED);
        v.set_ospeedr(6, gpio::vals::Ospeedr::HIGHSPEED);
        v.set_ospeedr(7, gpio::vals::Ospeedr::HIGHSPEED);
        v.set_ospeedr(8, gpio::vals::Ospeedr::HIGHSPEED);
    });
    GPIOA.pupdr().modify(|v| {
        // set pull-up/pull-down as no pull-up/pull-down
        v.set_pupdr(0, gpio::vals::Pupdr::FLOATING);
        v.set_pupdr(1, gpio::vals::Pupdr::FLOATING);
        v.set_pupdr(4, gpio::vals::Pupdr::FLOATING);
        v.set_pupdr(6, gpio::vals::Pupdr::FLOATING);
        v.set_pupdr(7, gpio::vals::Pupdr::FLOATING);
        v.set_pupdr(8, gpio::vals::Pupdr::FLOATING);
    });
    GPIOA.odr().modify(|v| {
        // set output as low
        v.set_odr(0, gpio::vals::Odr::LOW);
        v.set_odr(1, gpio::vals::Odr::LOW);
        v.set_odr(4, gpio::vals::Odr::LOW);
        v.set_odr(6, gpio::vals::Odr::LOW);
        v.set_odr(7, gpio::vals::Odr::LOW);
        v.set_odr(8, gpio::vals::Odr::LOW);
    });
}

/// set the thread pin high
#[allow(dead_code)]
#[inline]
pub fn thread_pin_high() {
    GPIOA.odr().modify(|v| {
        v.set_odr(0, gpio::vals::Odr::HIGH);
    });
}

/// set the thread pin low
#[allow(dead_code)]
#[inline]
pub fn thread_pin_low() {
    GPIOA.odr().modify(|v| {
        v.set_odr(0, gpio::vals::Odr::LOW);
    });
}

/// set the task() pin high
#[allow(dead_code)]
#[inline]
pub fn task_pin_high(task: usize) {
    critical_section::with(|_| {
        let task = match task {
            2 => 4,
            3 => 6,
            4 => 7,
            5 => 8,
            _ => 0,
        };
        GPIOA.odr().modify(|v| {
            v.set_odr(task, gpio::vals::Odr::HIGH);
        });
    });
}

/// set the task() pin low
#[allow(dead_code)]
#[inline]
pub fn task_pin_low(task: usize) {
    critical_section::with(|_| {
        let task = match task {
            2 => 4,
            3 => 6,
            4 => 7,
            5 => 8,
            _ => 0,
        };
        GPIOA.odr().modify(|v| {
            v.set_odr(task, gpio::vals::Odr::LOW);
        });
    });
}

/// set the interrupt pin low
#[allow(dead_code)]
#[inline]
pub fn interrupt_pin_low() {
    GPIOA.odr().modify(|v| {
        v.set_odr(1, gpio::vals::Odr::LOW);
    });
}

#[inline(never)]
pub fn delay(time: usize) {
    // 延时函数,time的单位约为0.5s，使用汇编编写从而不会被优化
    unsafe {
        asm!(
            // 先来个循环（总共是两层循环，内层循环次数8000000）
            "mov r0, #0",
            "1:",
            // 内层循环
            "mov r1, #0",
            "2:",
            "add r1, r1, #1",
            "cmp r1, r3",
            "blt 2b",
            // 外层循环
            "add r0, r0, #1",
            "cmp r0, r2",
            "blt 1b",
            in("r2") time,
            in("r3") 1000000/8,
        )
    }
}

#[allow(dead_code)]
#[inline]
pub fn led_on() {
    GPIOA.odr().modify(|v| {
        v.set_odr(5, gpio::vals::Odr::HIGH);
    });
}

#[allow(dead_code)]
#[inline]
pub fn led_off() {
    GPIOA.odr().modify(|v| {
        v.set_odr(5, gpio::vals::Odr::LOW);
    });
}