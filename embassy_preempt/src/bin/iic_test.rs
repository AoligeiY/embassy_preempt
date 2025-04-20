#![no_main]
#![no_std]
#![feature(impl_trait_in_assoc_type)]

// FFI接口
use core::ffi::c_void;
use core::arch::asm;

use embassy_preempt::executor::SyncOSTaskCreate;
use embassy_preempt::os_core::{OSInit, OSStart};
use embassy_preempt::os_time::OSTimeDly;
use embassy_preempt::pac::{gpio, GPIOA, GPIOC, RCC};

#[cfg(feature = "alarm_test")]
use defmt::info;

#[cortex_m_rt::entry]
fn iic_test() -> ! {
    led_init();
    iic_init();

    // os初始化
    OSInit();
    // 
    SyncOSTaskCreate(task1, 0 as *mut c_void, 0 as *mut usize, 10);
    SyncOSTaskCreate(task2, 0 as *mut c_void, 0 as *mut usize, 11);
    // 启动os
    OSStart();
}

fn task1(_args: *mut c_void) {
    loop {
        #[cfg(feature = "alarm_test")]
        info!("iic_test");
       
       OSTimeDly(400 * 100);
    }
}

fn task2(_args: *mut c_void) {
    loop {
        led_on();
        OSTimeDly(500 * 100);
        led_off();
        OSTimeDly(500 * 100);
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

// 向scl线写
pub fn iic_w_scl(value: u8) {
    if value > 0 {
        GPIOC.odr().modify(|f| {
            f.set_odr(2, gpio::vals::Odr::HIGH);
        });
    } else {
        GPIOC.odr().modify(|f| {
            f.set_odr(2, gpio::vals::Odr::LOW);
        });
    }
    delay(1);
}

// 向sda线写
pub fn iic_w_sda(value: u8) {
    if value > 0 {
        GPIOC.odr().modify(|f| {
            f.set_odr(3, gpio::vals::Odr::HIGH);
        });
    } else {
        GPIOC.odr().modify(|f| {
            f.set_odr(3, gpio::vals::Odr::LOW);
        });
    }
    delay(1);
}

// 读sda线
pub fn iic_r_sda() -> u8 {
    let value = GPIOC.idr().read().idr(3).to_bits();
    delay(1);
    value
}

pub fn iic_init() {
    RCC.ahb1enr().modify(|f| {
        f.set_gpiocen(true);
    });

    GPIOC.moder().modify(|f| {
       f.set_moder(2, gpio::vals::Moder::OUTPUT);
       f.set_moder(3, gpio::vals::Moder::OUTPUT); 
    });

    GPIOC.otyper().modify(|f| {
        f.set_ot(2, gpio::vals::Ot::OPENDRAIN);
        f.set_ot(3, gpio::vals::Ot::OPENDRAIN);
    });

    GPIOC.pupdr().modify(|f| {
        f.set_pupdr(2, gpio::vals::Pupdr::PULLUP);
        f.set_pupdr(3, gpio::vals::Pupdr::PULLUP);
    });

    GPIOC.ospeedr().modify(|f| {
        f.set_ospeedr(2, gpio::vals::Ospeedr::HIGHSPEED);
        f.set_ospeedr(3, gpio::vals::Ospeedr::HIGHSPEED);
    });

    // Set GPIOC pins 2 and 3 high
    GPIOC.odr().modify(|f| {
        f.set_odr(2, gpio::vals::Odr::HIGH);
        f.set_odr(3, gpio::vals::Odr::HIGH);
    });
}

pub fn iic_start() {
    iic_w_sda(1);
    iic_w_scl(1);
    iic_w_sda(0);
    iic_w_scl(0);
}

pub fn iic_stop() {
    iic_w_sda(0);
    iic_w_scl(1);
    iic_w_sda(1);
}

pub fn iic_sendbyte(_byte: u8) {
    for i in 0..8 {
        iic_w_sda(_byte & (0x80 >> i));
        iic_w_scl(1);
        iic_w_scl(0);
    }
}

pub fn iic_receive_byte() -> u8 {
    let mut byte = 0x00;
    iic_w_sda(1);

    for i in 0..8 {
        iic_w_scl(1);
        if iic_r_sda() == 1 {
            byte |= 0x80 >> i;
        }
        iic_w_scl(0);
    }
    byte
}

pub fn iic_send_ack(ack_bit: u8) {
    iic_w_sda(ack_bit);
    iic_w_scl(1);
    iic_w_scl(0);
}

pub fn iic_receive_ack() -> u8 {
    iic_w_sda(1);

    iic_w_scl(1);
    let ack_bit = iic_r_sda();
    iic_w_scl(0);
    ack_bit
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
            in("r3") 800000/8,
        )
    }
}