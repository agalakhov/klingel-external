use smart_leds::{SmartLedsWrite, White as W, RGBW};
use ws2812_uart;

use crate::hal::{self, serial, stm32};

use fugit::{Rate, Duration};

#[derive(Debug, Clone, Copy)]
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
                r: 138,
                g: 0,
                b: 90,
                a: W(0),
            },
            Yellow => RGBW {
                r: 150,
                g: 90,
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

#[derive(Debug)]
pub enum Mode {
    Constant(Color),
    Blink(Color, Rate<u32, 1, 100>),
}

impl Mode {
    const fn max_ticks(&self) -> u32 {
        use Mode::*;
        match self {
            Constant(_) => 1000,
            Blink(_, period) => period.raw(),
        }
    }

    fn color_for_tick(&self, tick: u32, on_reset: bool) -> Option<Color> {
        use Mode::*;
        match self {
            Constant(color) => {
                if tick == 0 || on_reset {
                    Some(*color)
                } else {
                    None
                }
            }
            Blink(color, period) => {
                let c = if tick >= period.raw() / 2 {
                    Color::Off
                } else {
                    *color
                };
                if tick == 0 || tick == period.raw() / 2 || on_reset {
                    Some(c)
                } else {
                    None
                }
            }
        }
    }
}

pub struct Leds {
    led: ws2812_uart::Ws2812<
        hal::serial::Serial<stm32::USART2, serial::BasicConfig>,
        ws2812_uart::device::Sk6812w,
    >,
    mode: Mode,
    tick: u32,
}

impl Leds {
    pub fn new(
        uart: hal::serial::Serial<stm32g0xx_hal::stm32::USART2, serial::BasicConfig>,
    ) -> Self {
        let led = ws2812_uart::Ws2812::<_, ws2812_uart::device::Sk6812w>::new(uart);

        let mode = Mode::Constant(Color::Magenta);
        Self { led, mode, tick: 0 }
    }

    pub const fn period(&self) -> Duration<u64, 1, 1000> {
        Duration::<u64, 1, 1000>::from_ticks(10)
    }

    pub fn tick(&mut self) {
        if let Some(color) = self.mode.color_for_tick(self.tick, false) {
            self.set_board_color_raw(color.into());
        }
        self.tick += 1;
        if self.tick >= self.mode.max_ticks() {
            self.tick = 0;
        };
    }

    fn set_board_color_raw(&mut self, color: RGBW<u8>) {
        self.led
            .write([color; 8].into_iter())
            .expect("Error sending LED color");
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        if self.tick >= self.mode.max_ticks() {
            self.tick = 0;
        }
        if let Some(color) = self.mode.color_for_tick(self.tick, true) {
            self.set_board_color_raw(color.into());
        }
    }
}
