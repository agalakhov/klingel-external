use smart_leds::{SmartLedsWrite, White as W, RGBW};
use ws2812_uart;

use crate::hal::{self, serial, stm32};

use bounded_integer::BoundedU8;
use fugit::Duration;

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

type RawColor = RGBW<u8>;

impl From<Color> for RawColor {
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

trait ScaleColor<Factor> {
    fn scale(self, factor: Factor) -> Self;
}

impl ScaleColor<(u8, u8)> for u8 {
    fn scale(self, (intensity, maximum): (u8, u8)) -> Self {
        ((self as u16) * (intensity as u16) / (maximum as u16)) as u8
    }
}

impl ScaleColor<(u32, u32)> for u8 {
    fn scale(self, (intensity, maximum): (u32, u32)) -> Self {
        ((self as u64) * (intensity as u64) / (maximum as u64)) as u8
    }
}

impl ScaleColor<Intensity> for u8 {
    fn scale(self, intensity: Intensity) -> Self {
        self.scale((intensity.get(), Intensity::MAX_VALUE))
    }
}

impl<T, Factor> ScaleColor<Factor> for RGBW<T>
where
    T: ScaleColor<Factor>,
    Factor: Copy,
{
    fn scale(self, factor: Factor) -> Self {
        let RGBW { r, g, b, a: W(w) } = self;
        RGBW {
            r: r.scale(factor),
            g: g.scale(factor),
            b: b.scale(factor),
            a: W(w.scale(factor)),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Mode {
    Constant(Color),
    Blink(Color, Duration<u32, 1, 100>),
    Glow(Color, Duration<u32, 1, 100>),
}

impl Mode {
    const fn max_ticks(&self) -> u32 {
        use Mode::*;
        match self {
            Constant(_) => 1000,
            Blink(_, period) => period.ticks(),
            Glow(_, period) => period.ticks(),
        }
    }

    fn color_for_tick(&self, tick: u32, on_reset: bool) -> Option<RawColor> {
        use Mode::*;
        match self {
            Constant(color) => {
                if tick == 0 || on_reset {
                    Some((*color).into())
                } else {
                    None
                }
            }
            Blink(color, period) => {
                let c = if tick >= period.ticks() / 2 {
                    Color::Off
                } else {
                    *color
                };
                if tick == 0 || tick == period.ticks() / 2 || on_reset {
                    Some(c.into())
                } else {
                    None
                }
            }
            Glow(color, period) => {
                let max = period.ticks() / 2;
                let f = if tick <= max {
                    tick
                } else {
                    period.ticks() - tick
                };
                Some(RawColor::from(*color).scale((f, max)))
            }
        }
    }
}

pub struct Leds {
    led: ws2812_uart::Ws2812<
        hal::serial::Serial<stm32::USART2, serial::BasicConfig>,
        ws2812_uart::device::Sk6812w,
    >,
    dirty: bool,
    mode: Mode,
    effect: Option<Mode>,
    tick: u32,
    intensity: Intensity,
}

impl Leds {
    pub fn new(
        uart: hal::serial::Serial<stm32g0xx_hal::stm32::USART2, serial::BasicConfig>,
    ) -> Self {
        let led = ws2812_uart::Ws2812::<_, ws2812_uart::device::Sk6812w>::new(uart);

        let mode = Mode::Constant(Color::Magenta);
        Self {
            led,
            mode,
            tick: 0,
            intensity: Intensity::MAX,
            effect: None,
            dirty: true,
        }
    }

    pub const fn period(&self) -> Duration<u64, 1, 1000> {
        Duration::<u64, 1, 1000>::from_ticks(10)
    }

    pub fn tick(&mut self) {
        self.tick += 1;
        self.refresh()
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.dirty = true;
    }

    pub fn set_intensity(&mut self, intensity: Intensity) {
        self.intensity = intensity;
        self.dirty = true;
    }

    pub fn show_effect(&mut self, effect: Mode) {
        self.tick = 0;
        self.effect = Some(effect);
        self.dirty = true;
    }

    fn refresh(&mut self) {
        let max = self
            .effect
            .map(|e| e.max_ticks())
            .unwrap_or(self.mode.max_ticks());
        if self.tick >= max {
            self.tick = 0;
            self.effect = None;
        };
        let mode = self.effect.as_ref().unwrap_or(&self.mode);
        if let Some(color) = mode.color_for_tick(self.tick, self.dirty) {
            self.set_board_color_raw(color);
        }
        self.dirty = false;
    }

    fn set_board_color_raw(&mut self, color: RawColor) {
        let color = color.scale(self.intensity);
        self.led
            .write([color; 8].into_iter())
            .expect("Error sending LED color");
    }
}
