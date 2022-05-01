#![no_std]
#![no_main]

mod adc;
mod led;
mod platform;
mod rs485;

use cortex_m_rt::entry;
use hal::prelude::*;
use stm32g0xx_hal as hal;

use core::fmt::Write;

use led::Color;
use platform::Platform;

#[entry]
fn main() -> ! {
    let Platform {
        mut rs485,
        mut delay,
        mut led,
        mut adc,
    } = Platform::startup();

    // LED test
    led.set_board_color(Color::Off);
    delay.delay(250.ms());
    for _ in 0..2 {
        for color in [
            Color::Red,
            Color::Green,
            Color::Blue,
            Color::Yellow,
            Color::Cyan,
            Color::Magenta,
            Color::White,
        ] {
            led.set_board_color(color);
            delay.delay(200.ms());
        }
    }
    led.set_board_color(Color::Off);

    writeln!(rs485, "# Door Bell 2.0 external board ready").ok();

    loop {
        delay.delay(250.ms());

        let (v, t) = adc.read_voltage_temperature();
        writeln!(rs485, "VDD: {} mv, T: {} degC", v, t).ok();

        if let Ok(Some(button)) = adc.read_button() {
            let i = button as u8;
            let color = match i {
                0 => Color::Off,
                1 => Color::Red,
                2 => Color::Green,
                3 => Color::Blue,
                4 => Color::Yellow,
                5 => Color::Cyan,
                _ => Color::White,
            };
            led.set_board_color(color);
        }
    }
}
