use crate::led::{Color, Mode, Intensity, Leds};
use fugit::{Duration, ExtU32};
use protocol::incoming::Message;

#[derive(Debug)]
pub struct Command {
    mode: Option<Mode>,
    effect: Option<Mode>,
    intensity: Option<Intensity>,
}

trait FromLetter: Sized {
    fn from_letter(letter: char) -> Option<Self>;
}

impl FromLetter for Mode {
    fn from_letter(letter: char) -> Option<Self> {
        match letter {
            'R' => Some(Mode::Constant(Color::Red)),
            'G' => Some(Mode::Constant(Color::Green)),
            'B' => Some(Mode::Constant(Color::Blue)),
            'W' => Some(Mode::Constant(Color::White)),
            'Y' => Some(Mode::Constant(Color::Yellow)),
            'M' => Some(Mode::Constant(Color::Magenta)),
            'C' => Some(Mode::Constant(Color::Cyan)),
            'r' => Some(Mode::Glow(Color::Red, slow_glow())),
            'g' => Some(Mode::Glow(Color::Green, slow_glow())),
            'b' => Some(Mode::Glow(Color::Blue, slow_glow())),
            'w' => Some(Mode::Glow(Color::White, slow_glow())),
            'y' => Some(Mode::Glow(Color::Yellow, slow_glow())),
            'm' => Some(Mode::Glow(Color::Magenta, slow_glow())),
            'c' => Some(Mode::Glow(Color::Cyan, slow_glow())),
            _ => None,
        }
    }
}

fn slow_glow() -> Duration<u32, 1, 100> {
    300_u32.millis()
}

impl Command {
    pub fn apply(&self, leds: &mut Leds) {
        if let Some(mode) = self.mode {
            leds.set_mode(mode);
        }

        if let Some(effect) = self.effect {
            leds.show_effect(effect);
        }

        if let Some(intensity) = self.intensity {
            leds.set_intensity(intensity);
        }
    }

    pub fn from_rs485(message: Message) -> Option<Self> {
        if message.color.is_some() || message.effect.is_some() || message.intensity.is_some() {
            let mode = message.color.and_then(Mode::from_letter);
            let effect = message.effect.and_then(Mode::from_letter);
            let intensity = message.intensity.and_then(Intensity::new);

            Some(Command {
                mode,
                effect,
                intensity,
            })
        } else {
            None
        }
    }

    pub fn no_connection() -> Self {
        Command {
            mode: Some(Mode::Blink(Color::Red, 1.secs())),
            effect: Some(Mode::Glow(Color::Blue, slow_glow())),
            intensity: None
        }
    }
}
