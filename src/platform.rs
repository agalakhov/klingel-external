use panic_semihosting as _;

use crate::hal::{
    self,
    prelude::*,
    rcc::{self, Enable, PllConfig},
    serial, stm32,
};

use crate::adc::AdcReader;
use crate::led::Leds;

pub struct Platform {
    pub delay: hal::timer::delay::Delay<stm32::SYST>,
    pub rs485: hal::serial::Serial<stm32::USART1, serial::FullConfig>,
    pub led: Leds,
    pub adc: AdcReader<hal::gpio::gpioa::PA13<hal::gpio::Analog>>,
}

impl Platform {
    pub fn startup() -> Self {
        let core = cortex_m::Peripherals::take().unwrap();
        let dev = stm32::Peripherals::take().unwrap();

        // Configure system clocks
        let pll_cfg = PllConfig::with_hsi(2, 16, 2); // 64 MHz
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

        // Configure RS485
        let (rxd, txd, de) = (gpiob.pb7, gpioa.pa9, gpioa.pa12); // USART1
        let rs485 = dev
            .USART1
            .usart_hwflow(
                txd,
                rxd,
                serial::Rs485FlowControl { de },
                serial::FullConfig::default()
                    .baudrate(115200.bps())
                    .invert_tx()
                    .invert_rx()
                    .swap_pins(),
                &mut rcc,
            )
            .expect("Can't initialize RS485");

        // Sleep 50 milliseconds before disabling SWD which is used as UART TX and ADC input.
        // This helps doing SWD debugging.
        delay.delay(50.ms());

        // Configure Smart LED using UART
        let led = gpioa.pa14; // USART2
        let led = dev
            .USART2
            .usart(
                led,
                serial::NoRxPin,
                serial::BasicConfig::default()
                    .baudrate(3_750_000.bps())
                    .invert_tx(),
                &mut rcc,
            )
            .expect("Can't initialize LED UART");
        let led = Leds::new(led);

        // Configure buttons via ADC
        let buttons = gpioa.pa13; // ADC1_IN17
        let adc = dev.ADC.constrain(&mut rcc);
        let adc = AdcReader::new(adc, buttons, &mut delay);

        Self {
            delay,
            rs485,
            led,
            adc,
        }
    }
}
