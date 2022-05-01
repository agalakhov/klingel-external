use smart_leds::{SmartLedsWrite, White as W, RGBW};
use ws2812_uart;

use crate::hal::{self, serial, stm32};

#[derive(Debug)]
pub enum Color {
    Off,
    Red,
    Green,
    Blue,
    Cyan,
    Magenta,
    Yellow,
    White,
}

impl From<Color> for RGBW<u8> {
    fn from(color: Color) -> Self {
        use Color::*;
        match color {
            Off => RGBW {
                r: 0,
                g: 0,
                b: 0,
                a: W(0),
            },
            Red => RGBW {
                r: 255,
                g: 0,
                b: 0,
                a: W(0),
            },
            Green => RGBW {
                r: 0,
                g: 255,
                b: 0,
                a: W(0),
            },
            Blue => RGBW {
                r: 0,
                g: 0,
                b: 255,
                a: W(0),
            },
            Cyan => RGBW {
                r: 0,
                g: 128,
                b: 128,
                a: W(0),
            },
            Magenta => RGBW {
                r: 128,
                g: 0,
                b: 128,
                a: W(0),
            },
            Yellow => RGBW {
                r: 128,
                g: 128,
                b: 0,
                a: W(0),
            },
            White => RGBW {
                r: 0,
                g: 0,
                b: 0,
                a: W(255),
            },
        }
    }
}

pub struct Leds {
    led: ws2812_uart::Ws2812<
        hal::serial::Serial<stm32::USART2, serial::BasicConfig>,
        ws2812_uart::device::Sk6812w,
    >,
}

impl Leds {
    pub fn new(
        uart: hal::serial::Serial<stm32g0xx_hal::stm32::USART2, serial::BasicConfig>,
    ) -> Self {
        let led = ws2812_uart::Ws2812::<_, ws2812_uart::device::Sk6812w>::new(uart);

        Self { led }
    }

    pub fn tick(&mut self) {

    }


    fn set_board_color_raw(&mut self, color: RGBW<u8>) {
        self.led
            .write([color; 8].into_iter())
            .expect("Error sending LED color");
    }

    pub fn set_board_color(&mut self, color: Color) {
        self.set_board_color_raw(color.into())
    }
}


