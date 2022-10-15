use rtic::app;

use panic_semihosting as _;

#[app(device = crate::hal::stm32, peripherals = true, dispatchers = [EXTI0_1, EXTI2_3, EXTI4_15, I2C1, I2C2, SPI1, SPI2])]
mod app {
    use crate::adc::{AdcReader, Button};
    use crate::hal::{
        gpio::{gpioa::PA13, Analog},
        prelude::*,
        rcc::{self, Enable, PllConfig},
        serial,
        stm32,
        watchdog::IndependedWatchdog,
    };
    use crate::led::{Color, Leds, Mode};
    use crate::rs485::Rs485;
    use crate::command::Command;
    use core::mem::replace;
    use rtic::pend;
    use systick_monotonic::{fugit::ExtU64, Systick};
    use protocol::outgoing::Message;
    use cortex_m::asm;

    #[shared]
    struct Shared {
        voltage: u16,
        temperature: i16,
        button: Option<Button>,
        command: Option<Command>,
        timer_flag: bool,
        ping_flag: bool,
    }

    #[local]
    struct Local {
        led: Leds,
        adc: AdcReader<PA13<Analog>>,
        rs485: Rs485,
        dog: IndependedWatchdog, 
    }

    #[monotonic(binds = SysTick, default = true)]
    type MyMono = Systick<1000>; // 1000 Hz / 1 ms granularity

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
        let uart = dev
            .USART1
            .usart(
                (txd, rxd, de),
                serial::FullConfig::default()
                    .baudrate(crate::RS485_BAUD.bps())
                    .invert_tx()
                    .invert_rx()
                    .swap_pins(),
                &mut rcc,
            )
            .expect("Can't initialize RS485");
        let rs485timer = dev.TIM17.timer(&mut rcc);
        let dma = dev.DMA.split(&mut rcc, dev.DMAMUX);
        let rs485 = Rs485::new(uart, rs485timer, dma.ch1);

        // Sleep 50 milliseconds before disabling SWD which is used as UART TX and ADC input.
        // This helps doing SWD debugging.
        delay.delay(50_u32.millis());

        // Setup watchdog
        let mut dog = dev.IWDG.constrain();
        dog.start(10_u32.secs());

        // Configure Smart LED using UART
        let led = gpioa.pa14; // USART2
        let led = dev
            .USART2
            .usart(
                (led, serial::NoRx),
                serial::BasicConfig::default()
                    .baudrate(3_750_000.bps())
                    .invert_tx(),
                &mut rcc,
            )
            .expect("Can't initialize LED UART");
        let mut led = Leds::new(led);
        delay.delay(1_u32.millis());
        led.set_mode(Mode::Blink(Color::Yellow, 1_u32.secs()));

        // Configure buttons via ADC
        let buttons = gpioa.pa13; // ADC1_IN17
        let adc = dev.ADC.constrain(&mut rcc);
        let adc = AdcReader::new(adc, buttons, &mut delay);

        let shared = Shared {
            voltage: 0,
            temperature: -2731,
            button: None,
            command: None,
            ping_flag: false,
            timer_flag: false,
        };

        let local = Local { led, adc, rs485, dog };
        let mono = Systick::new(delay.release(), rcc.clocks.ahb_clk.raw());

        led_work::spawn().expect("Can't spawn led_work");
        adc_work::spawn().expect("Can't spawn adc_work");
        ping::spawn_after(1000_u64.millis()).expect("Can't spawn ping");

        (shared, local, init::Monotonics(mono))
    }

    #[task(priority = 2, local = [led], shared = [command])]
    fn led_work(mut cx: led_work::Context) {
        let cmd = cx.shared.command.lock(|cmd| cmd.take());
        if let Some(cmd) = cmd {
            cmd.apply(&mut cx.local.led);
        } else {
            cx.local.led.tick();
        }
        led_work::spawn_after(cx.local.led.period()).expect("Can't respawn led_work");
    }

    #[task(priority = 1, local = [adc], shared = [voltage, temperature, button])]
    fn adc_work(mut cx: adc_work::Context) {
        let adc = cx.local.adc;
        let (voltage, temperature) = adc.read_voltage_temperature();
        cx.shared.voltage.lock(|v| {
            *v = voltage;
        });
        cx.shared.temperature.lock(|t| {
            *t = temperature;
        });
        if let Ok(Some(button)) = adc.read_button() {
            cx.shared.button.lock(|b| {
                *b = Some(button);
            });
        }
        adc_work::spawn_after(1_u64.millis()).expect("Can't respawn adc_work");
    }

    #[task(priority = 1, shared = [ping_flag])]
    fn ping(mut cx: ping::Context) {
        cx.shared.ping_flag.lock(|f| *f = true);
        ping::spawn_after(1000_u64.millis()).expect("Can't respawn ping");
    }

    #[task(priority = 2, binds = TIM17, shared = [timer_flag])]
    fn timer_interrupt(mut cx: timer_interrupt::Context) {
        cx.shared.timer_flag.lock(|v| *v = true);
        pend(stm32::Interrupt::USART1);
    }

    #[task(priority = 3, binds = USART1, local = [dog, rs485], shared = [button, voltage, temperature, ping_flag, timer_flag, command])]
    fn rs485_interrupt(mut cx: rs485_interrupt::Context) {
        cx.local.dog.feed();
        let cmd = cx.local.rs485.interrupt(
            cx.shared.timer_flag.lock(|f| replace(f, false)),
            |buf| {
                let button = cx.shared.button.lock(|b| b.take());
                let ping_flag = cx.shared.ping_flag.lock(|f| replace(f, false));
                if button.is_some() || ping_flag {
                    let voltage = cx.shared.voltage.lock(|v| *v);
                    let temperature = cx.shared.temperature.lock(|t| *t);
                    let message = Message {
                        sender: crate::DEVICE_ADDRESS,
                        button: button.map(|b| b as u8),
                        temperature,
                        voltage,
                    };
                    message.to_bytes(buf);
                    true
                } else {
                    false
                }
            }
        );
        if let Some(c) = cmd {
            cx.shared.command.lock(|cmd| {
                *cmd = Some(c);
            });
        }
    }

    #[idle]
    fn idle(_cx: idle::Context) -> ! {
        loop {
            asm::wfi();
        }
    }
}
