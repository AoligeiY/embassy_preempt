#![no_main]
#![no_std]
#![feature(impl_trait_in_assoc_type)]

// FFI接口
use core::ffi::c_void;

use embassy_preempt::os_task::SyncOSTaskCreate;
use embassy_preempt::os_core::{OSInit, OSStart};
use embassy_preempt::os_time::OSTimeDly;
use embassy_preempt::pac::{usart, gpio, GPIOA, RCC, USART1};

#[cfg(feature = "alarm_test")]
use defmt::info;

#[cortex_m_rt::entry]
fn usart_test() -> ! {
    led_init();
    usart_init();

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
        info!("usart_test");
       usart_send_byte(b'A');
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

// 波特率、时钟
const BAUD_RATE: u64 = 115200;
const CLOCK: u64 = 84_000_000;
static USART_DIV: u16 = (CLOCK / BAUD_RATE) as u16;

#[allow(dead_code)]
fn usart_init() {
    #[cfg(feature = "alarm_test")]
    info!("usart_init");

    // 启用 GPIOA 和 USART1 的时钟
    RCC.ahb1enr().modify(|f| {
        f.set_gpioaen(true);
    });
    RCC.apb2enr().modify(|f| {
        f.set_usart1en(true);
    });

    // 配置 GPIOA 的引脚 PA9 (TX) 和 PA10 (RX) 为复用功能模式
    GPIOA.moder().modify(|f| {
        f.set_moder(9, gpio::vals::Moder::ALTERNATE);
        f.set_moder(10, gpio::vals::Moder::ALTERNATE);
    });
    
    GPIOA.afr(1).modify(|f| {
        f.set_afr(1, 7);    // PA9 复用为 USART1_TX
        f.set_afr(2, 7);    // PA10 复用为 USART1_RX
    });

    USART1.brr().write(|f| {
        f.set_brr(USART_DIV);   // 设置波特率
    });

    USART1.cr1().modify(|f| {
        f.set_ue(true);         // 启用 USART1
        f.set_m0(usart::vals::M0::BIT8);    // 8 位数据位
        f.set_te(true);         // 启用发送
        f.set_re(true);         // 启用接收
    });

    USART1.cr2().modify(|f| {
        f.set_stop(usart::vals::Stop::STOP1);   // 1 位停止位
    });
}

#[allow(dead_code)]
fn usart_send_byte(data: u8) {
    #[cfg(feature = "alarm_test")]
        info!("usart_test_send");
    while !USART1.sr().read().txe() {}
    USART1.dr().write(|f| f.set_dr(data as u16 & 0x01FF));
}

#[allow(dead_code)]
fn usart_receive_byte() -> u8{
    // while !USART1.sr().read().rxne() {}
    (USART1.dr().read().dr() & 0x01FF) as u8
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