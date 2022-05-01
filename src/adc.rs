use crate::hal::{
    analog::adc::{Adc, OversamplingRatio, Precision, SampleTime},
    hal::adc::Channel,
    hal::blocking::delay::DelayUs,
    prelude::*,
};

use core::mem::replace;

pub enum Button {
    Button0 = 0,
    Button1 = 1,
    Button2 = 2,
    Button3 = 3,
    Button4 = 4,
    Button5 = 5,
}

pub struct AdcReader<PIN> {
    adc: Adc,
    pin: PIN,
    last_reading: u32,
}

pub struct BadButton;

impl<PIN> AdcReader<PIN>
where
    PIN: Channel<Adc, ID = u8>,
{
    pub fn new(mut adc: Adc, pin: PIN, delay: &mut impl DelayUs<u8>) -> Self {
        adc.set_sample_time(SampleTime::T_80);
        adc.set_precision(Precision::B_12);
        adc.set_oversampling_ratio(OversamplingRatio::X_16);
        adc.set_oversampling_shift(4);
        adc.oversampling_enable(true);

        delay.delay_us(20); // Wait for ADC voltage regulator to stabilize
        adc.calibrate();

        Self {
            adc,
            pin,
            last_reading: 0,
        }
    }

    pub fn read_button(&mut self) -> Result<Option<Button>, BadButton> {
        const TOLERANCE: u32 = 5;

        let u = self.adc.read(&mut self.pin).expect("ADC input read failed");

        let lr = replace(&mut self.last_reading, u);
        if u.abs_diff(lr) > TOLERANCE {
            return Ok(None);
        }

        use Button::*;
        match u {
            1328..=1528 => Ok(Some(Button0)),
            1606..=1806 => Ok(Some(Button1)),
            1873..=2073 => Ok(Some(Button2)),
            2180..=2380 => Ok(Some(Button3)),
            2450..=2650 => Ok(Some(Button4)),
            2730..=2930 => Ok(Some(Button5)),
            3963..=4095 => Ok(None),
            _ => Err(BadButton),
        }
    }

    pub fn read_voltage_temperature(&mut self) -> (u16, i16) {
        let voltage = self.adc.read_vref().expect("ADC VDDA read failed");
        let temperature = self.adc.read_temperature().expect("ADC VTemp read failed");
        (voltage, temperature)
    }
}
