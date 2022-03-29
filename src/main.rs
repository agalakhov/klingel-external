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
use ws2812_spi as led;

use core::fmt::Write;

struct NoRx;
impl<T> serial::RxPin<T> for NoRx {
    fn setup(&self) {}
}

#[entry]
fn main() -> ! {
    let core = cortex_m::Peripherals::take().unwrap();
    let dev = stm32::Peripherals::take().unwrap();

    let pll_cfg = PllConfig::with_hsi(4, 24, 2);
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

    //    let led = spi::NoMosi;
    //    let led = gpiob.pb5; // SPI1
    let led = gpioa.pa14; // USART2
                          //    let mut spi = Spi::spi1(dev.SPI1, (spi::NoSck, spi::NoMiso, led), led::MODE, 3.mhz(), &mut rcc);
                          //    let mut led = led::Ws2812::new_sk6812w(spi);
                          //
    let mut led = dev
        .USART2
        .usart(
            led,
            NoRx,
            serial::BasicConfig::default()
                .baudrate(3_000_000.bps())
                .wordlength_7()
                .invert_tx(),
            &mut rcc,
        )
        .expect("Can't initialize LED UART");

    writeln!(rs485, "# LED work started").ok();

    let _buttons = gpioa.pa13; // ADC1_IN17

    loop {
        block!(led.write(0b100_100));
        for i in 0..24 {
            if let Err(e) = block!(led.write(0b11001000)) {
                writeln!(rs485, "W err @{}: {:?}", i, e).ok();
            }
        }
        delay.delay(80.us());
    }
}
