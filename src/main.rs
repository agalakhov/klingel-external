#![no_std]
#![no_main]

use panic_semihosting as _;

use cortex_m_rt::entry;
use stm32g0xx_hal as hal;

use crate::hal::{
    block,
    prelude::*,
    rcc::{self, Enable, PllConfig},
    serial,
    spi::{self, Spi},
    stm32,
};

use smart_leds::{SmartLedsWrite, White, RGBW};
use ws2812_uart as led;

use core::fmt::Write;

struct NoRx;
impl<T> serial::RxPin<T> for NoRx {
    fn setup(&self) {}
}

#[entry]
fn main() -> ! {
    let core = cortex_m::Peripherals::take().unwrap();
    let dev = stm32::Peripherals::take().unwrap();

    let pll_cfg = PllConfig::with_hsi(2, 16, 2);
    let rcc_cfg = rcc::Config::pll().pll_cfg(pll_cfg);
    let mut rcc = dev.RCC.freeze(rcc_cfg);

    // Remap PA11/PA9 as PA9
    stm32::SYSCFG::enable(&mut rcc);
    unsafe {
        dev.SYSCFG
            .cfgr1
            .modify(|r, w| w.bits(r.bits() | (0b_0_1_000)));
    }

    let mut delay = core.SYST.delay(&mut rcc);

    let gpioa = dev.GPIOA.split(&mut rcc);
    let gpiob = dev.GPIOB.split(&mut rcc);

    let (rxd, txd, rts) = (gpiob.pb7, gpioa.pa9, gpioa.pa12); // USART1
    let mut rs485 = dev
        .USART1
        .usart(
            txd,
            rxd,
            serial::FullConfig::default()
                .baudrate(115200.bps())
                .invert_tx()
                .invert_rx()
                .swap_pins(),
            &mut rcc,
        )
        .expect("Can't initialize RS485");
    let mut rts = rts.into_push_pull_output();
    rts.set_high().ok();

    writeln!(rs485, "# Door Bell 2.0 external board ready").ok();

    // Sleep 3 seconds before initializing SWD port as GPIO
    delay.delay(3.seconds());

    let led = gpioa.pa14; // USART2
    let led = dev
        .USART2
        .usart(
            led,
            NoRx,
            serial::BasicConfig::default()
                .baudrate(3_750_000.bps())
                .invert_tx(),
            &mut rcc,
        )
        .expect("Can't initialize LED UART");
    let mut led = led::Ws2812::<_, led::device::Sk6812w>::new(led);

    writeln!(rs485, "# LED work started").ok();

    let _buttons = gpioa.pa13; // ADC1_IN17

    let mut i = 0;
    loop {
        let color = RGBW {
            r: if i & 1 != 0 { 255 } else { 0 },
            g: if i & 2 != 0 { 255 } else { 0 },
            b: if i & 4 != 0 { 255 } else { 0 },
            a: White(if i & 8 != 0 { 255 } else { 0 }),
        };
        i = (i + 1) % 16;

        writeln!(rs485, "Setting color {:?}", color);
        led.write([color; 8].into_iter())
            .expect("Error sending LED color");

        delay.delay(250.ms());
    }
}
