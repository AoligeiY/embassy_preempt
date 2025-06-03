#![no_std]
#![no_main]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

#![allow(non_camel_case_types)]
use cortex_m::peripheral::scb::SystemHandler;
#[allow(unused_imports)]
use cortex_m::peripheral::{NVIC, SCB, SYST, syst};
use stm32_metapac::timer::TimGp16;
#[cfg(any(feature = "alarm_test", feature = "defmt"))]
#[allow(unused_imports)]
use defmt::{info,trace};


/// the timer used as the time Driver
#[cfg(feature = "time_driver_tim1")]
const TIMER: TimGp16 = stm32_metapac::TIM1;
#[cfg(feature = "time_driver_tim2")]
const TIMER: TimGp32 = stm32_metapac::TIM2;
#[cfg(feature = "time_driver_tim3")]
// by noah: in current project, we use Timer 3 as the time driver
/// set the time driver to be Timer3
pub const TIMER: TimGp16 = stm32_metapac::TIM3;
#[cfg(feature = "time_driver_tim4")]
const TIMER: TimGp16 = stm32_metapac::TIM4;
#[cfg(feature = "time_driver_tim5")]
const TIMER: TimGp32 = stm32_metapac::TIM5;
#[cfg(feature = "time_driver_tim8")]
const TIMER: TimGp16 = stm32_metapac::TIM8;
#[cfg(feature = "time_driver_tim9")]
const TIMER: TimGp16 = stm32_metapac::TIM9;
#[cfg(feature = "time_driver_tim12")]
const TIMER: TimGp16 = stm32_metapac::TIM12;
#[cfg(feature = "time_driver_tim15")]
const TIMER: TimGp16 = stm32_metapac::TIM15;
#[cfg(feature = "time_driver_tim20")]
const TIMER: TimGp16 = stm32_metapac::TIM20;
#[cfg(feature = "time_driver_tim21")]
const TIMER: TimGp16 = stm32_metapac::TIM21;
#[cfg(feature = "time_driver_tim22")]
const TIMER: TimGp16 = stm32_metapac::TIM22;
#[cfg(feature = "time_driver_tim23")]
const TIMER: TimGp16 = stm32_metapac::TIM23;
#[cfg(feature = "time_driver_tim24")]
const TIMER: TimGp16 = stm32_metapac::TIM24;

///the language items
pub mod lang_items;
// pub mod os_cpu;
/// the time driver
pub mod time_driver;

/*
********************************************************************************************************************************************
*                                                               critical section
********************************************************************************************************************************************
*/
use cortex_m::{interrupt, Peripherals};
use cortex_m::register::primask;
use critical_section::{set_impl, Impl, RawRestoreState};

struct SingleCoreCriticalSection;
set_impl!(SingleCoreCriticalSection);

unsafe impl Impl for SingleCoreCriticalSection {
    unsafe fn acquire() -> RawRestoreState {
        let was_active = primask::read().is_active();
        interrupt::disable();
        was_active
    }

    unsafe fn release(was_active: RawRestoreState) {
        // Only re-enable interrupts if they were enabled before the critical section.
        if was_active {
            interrupt::enable()
        }
    }
}

/*
********************************************************************************************************************************************
*                                                          core peripherals init
********************************************************************************************************************************************
*/

/// by noah: init the core peripherals. For the task() just can be called **once**, we should init the core peripherals together
pub fn init_core_peripherals() {
    let mut p = Peripherals::take().unwrap();
    // set the NVIC
    unsafe{
        // set the group as 2-2
        let aircr = p.SCB.aircr.read();
        let mut aircr = aircr & !(0b1111 << 8);
        aircr = aircr | (0b101 << 8);
        p.SCB.aircr.write(aircr);
        // infer that the group is 2-2
        // set the TIM3 prio as 3
        #[cfg(feature = "defmt")]
        info!("the prio of TIM3 is {}",NVIC::get_priority(stm32_metapac::Interrupt::TIM3));

        #[cfg(feature = "time_driver_tim1")]{
            p.NVIC.set_priority(stm32_metapac::Interrupt::TIM1_CC, 32);
            p.NVIC.set_priority(stm32_metapac::Interrupt::TIM1_UP_TIM10, 32);
        }
        #[cfg(feature = "time_driver_tim2")]
        p.NVIC.set_priority(stm32_metapac::Interrupt::TIM2, 32);
        #[cfg(feature = "time_driver_tim3")]
        p.NVIC.set_priority(stm32_metapac::Interrupt::TIM3, 32);
        #[cfg(feature = "time_driver_tim4")]
        p.NVIC.set_priority(stm32_metapac::Interrupt::TIM4, 32);
        #[cfg(feature = "time_driver_tim5")]
        p.NVIC.set_priority(stm32_metapac::Interrupt::TIM5, 32);
        #[cfg(feature = "time_driver_tim8")]
        p.NVIC.set_priority(stm32_metapac::Interrupt::TIM8, 32);
        #[cfg(feature = "time_driver_tim9")]
        p.NVIC.set_priority(stm32_metapac::Interrupt::TIM9, 32);
        #[cfg(feature = "time_driver_tim12")]
        p.NVIC.set_priority(stm32_metapac::Interrupt::TIM12, 32);
        #[cfg(feature = "time_driver_tim15")]
        p.NVIC.set_priority(stm32_metapac::Interrupt::TIM15, 32);
        #[cfg(feature = "time_driver_tim20")]
        p.NVIC.set_priority(stm32_metapac::Interrupt::TIM20, 32);
        #[cfg(feature = "time_driver_tim21")]
        p.NVIC.set_priority(stm32_metapac::Interrupt::TIM21, 32);
        #[cfg(feature = "time_driver_tim22")]
        p.NVIC.set_priority(stm32_metapac::Interrupt::TIM22, 32);
        #[cfg(feature = "time_driver_tim23")]
        p.NVIC.set_priority(stm32_metapac::Interrupt::TIM23, 32);
        #[cfg(feature = "time_driver_tim24")]
        p.NVIC.set_priority(stm32_metapac::Interrupt::TIM24, 32);

        

        #[cfg(feature = "defmt")]
        info!("the prio of TIM3 is {}",NVIC::get_priority(stm32_metapac::Interrupt::TIM3));

        #[cfg(feature = "defmt")]
        info!("the prio of EXTI15_10 is {}",NVIC::get_priority(stm32_metapac::Interrupt::EXTI15_10));
        // set the EXTI13 prio as 1
        p.NVIC.set_priority(stm32_metapac::Interrupt::EXTI15_10, 16);
        #[cfg(feature = "defmt")]
        info!("the prio of EXTI15_10 is {}",NVIC::get_priority(stm32_metapac::Interrupt::EXTI15_10));
        #[cfg(feature = "defmt")]
        info!("the prio of PendSV is {}",SCB::get_priority(SystemHandler::PendSV));
        p.SCB.set_priority(SystemHandler::PendSV, 0xf<<4);
        #[cfg(feature = "defmt")]
        info!("the prio of PendSV is {}",SCB::get_priority(SystemHandler::PendSV));
    }
}

// const SYSCLK_Frequency: u32 = 84_000_000;
// const OS_TICKS_PER_SEC: u32 = 1_000;

// /// init systick
// pub fn init_systick() {

//     let mut p = Peripherals::take().unwrap();

//     unsafe {
//         // clear cnt value
//         p.SYST.clear_current();

//         // set the ARR to 84000 
//         p.SYST.set_reload(SYSCLK_Frequency/OS_TICKS_PER_SEC);   // 1ms

//         // set SYSTICK's priority to 14
//         p.SCB.set_priority(SystemHandler::SysTick, 0xe<<4); // 240

//         // selecting the clock source
//         p.SYST.set_clock_source(syst::SystClkSource::Core); // 84M

//         // enable systick counter
//         p.SYST.enable_counter();

//         // enable systick interrupt
//         p.SYST.enable_interrupt();

//     }
// }
// #[warn(unused_unsafe)]
// /// test systick whether or not work
// pub fn test_systick() {

//     unsafe {
        
//         #[cfg(feature = "alarm_test")]
//         info!("systick count is {}", SYST::get_current());
//         #[cfg(feature = "alarm_test")]
//         info!("systick count is {}", SYST::get_reload());
       
//     }
// }

// use cortex_m_rt::exception;
// #[exception]
// fn SysTick() {
    
// }