use rtic::app;

use panic_semihosting as _;

#[app(device = crate::hal::stm32, peripherals = true, dispatchers = [EXTI0_1, EXTI2_3, EXTI4_15])]
mod app {
    use crate::adc::{AdcReader, Button};
    use crate::hal::{
        self,
        prelude::*,
        gpio::{gpioa::PA13, Analog},
        rcc::{self, Enable, PllConfig},
        serial, stm32,
    };
    use crate::led::{Color, Leds, Mode};
    use systick_monotonic::{fugit::ExtU64, Systick};

    #[shared]
    struct Shared {
        voltage: u16,
        temperature: i16,
        button: Option<Button>,
    }

    #[local]
    struct Local {
        led: Leds,
        adc: AdcReader<PA13<Analog>>,
    }

    #[monotonic(binds = SysTick, default = true)]
    type MyMono = Systick<100>; // 100 Hz / 10 ms granularity

    #[init]
    fn init(cx: init::Context) -> (Shared, Local, init::Monotonics) {
        let core = cx.core;
        let dev = cx.device;

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
        let mut led = Leds::new(led);
        delay.delay(1.ms());
        led.set_mode(Mode::Blink(Color::Yellow, 500));

        // Configure buttons via ADC
        let buttons = gpioa.pa13; // ADC1_IN17
        let adc = dev.ADC.constrain(&mut rcc);
        let adc = AdcReader::new(adc, buttons, &mut delay);

        let shared = Shared {
            voltage: 0,
            temperature: -273,
            button: None,
        };

        let local = Local { led, adc };
        let mono = Systick::new(delay.release(), rcc.clocks.core_clk.0);

        led_work::spawn().expect("Can't spawn led_work");
        adc_work::spawn().expect("Can't spawn adc_work");

        (shared, local, init::Monotonics(mono))
    }

    #[task(priority = 1, local = [led], shared = [button])]
    fn led_work(mut cx: led_work::Context) {
        let button = cx.shared.button.lock(|mut b| {
            b.take()
        });

        if let Some(button) = button {
            use Button::*;
            let color = match button {
                Button0 => Color::White,
                Button1 => Color::Red,
                Button2 => Color::Green,
                Button3 => Color::Blue,
                Button4 => Color::Yellow,
                Button5 => Color::Magenta,
            };
            cx.local.led.set_mode(Mode::Constant(color));
        }

        cx.local.led.tick();
        led_work::spawn_after(10.millis()).expect("Can't respawn led_work");
    }

    #[task(priority = 1, local = [adc], shared = [voltage, temperature, button])]
    fn adc_work(mut cx: adc_work::Context) {
        let adc = cx.local.adc;
        let (voltage, temperature) = adc.read_voltage_temperature();
        cx.shared.voltage.lock(|mut v| {
            *v = voltage;
        });
        cx.shared.temperature.lock(|mut t| {
            *t = temperature;
        });
        if let Ok(Some(button)) = adc.read_button() {
            cx.shared.button.lock(|mut b| {
                *b = Some(button);
            });
        }
        adc_work::spawn_after(1.millis()).expect("Can't respawn adc_work");
    }

    #[idle]
    fn idle(_cx: idle::Context) -> ! {
        loop {}
    }
}
