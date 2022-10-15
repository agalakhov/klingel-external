use smart_leds::{SmartLedsWrite, White as W, RGBW};
use ws2812_uart;

use crate::hal::{self, serial, stm32};

use fugit::{Rate, Duration};
use bounded_integer::BoundedU8;

pub type Intensity = BoundedU8<1, 10>;

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

trait ScaleColor {
    fn scale(self, intensity: Intensity) -> Self;
}

impl ScaleColor for u8 {
    fn scale(self, intensity: Intensity) -> Self {
        ((self as u16) * (Intensity::MAX_VALUE as u16) / (intensity.get() as u16)) as u8
    }
}

impl<T: ScaleColor> ScaleColor for RGBW<T> {
    fn scale(self, intensity: Intensity) -> Self {
        let RGBW { r, g, b, a: W(w) } = self;
        RGBW {
            r: r.scale(intensity),
            g: g.scale(intensity),
            b: b.scale(intensity),
            a: W(w.scale(intensity)),
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
    intensity: Intensity,
}

impl Leds {
    pub fn new(
        uart: hal::serial::Serial<stm32g0xx_hal::stm32::USART2, serial::BasicConfig>,
    ) -> Self {
        let led = ws2812_uart::Ws2812::<_, ws2812_uart::device::Sk6812w>::new(uart);

        let mode = Mode::Constant(Color::Magenta);
        Self { led, mode, tick: 0, intensity: Intensity::MAX }
    }

    pub const fn period(&self) -> Duration<u64, 1, 1000> {
        Duration::<u64, 1, 1000>::from_ticks(10)
    }

    pub fn tick(&mut self) {
        self.tick += 1;
        self.refresh(false)
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.refresh(true);
    }

    pub fn set_intensity(&mut self, intensity: Intensity) {
        self.intensity = intensity;
        self.refresh(true);
    }

    fn refresh(&mut self, force: bool) {
        if self.tick >= self.mode.max_ticks() {
            self.tick = 0;
        };
        if let Some(color) = self.mode.color_for_tick(self.tick, force) {
            self.set_board_color_raw(color.into());
        }
    }

    fn set_board_color_raw(&mut self, color: RGBW<u8>) {
        let color = color.scale(self.intensity);
        self.led
            .write([color; 8].into_iter())
            .expect("Error sending LED color");
    }
}
