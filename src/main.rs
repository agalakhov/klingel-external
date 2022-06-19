#![no_std]
#![no_main]

mod adc;
mod app;
mod led;
mod rs485;

use stm32g0xx_hal as hal;

pub(crate) const RS485_BAUD: u32 = 115200;
pub(crate) const MAX_ADDRESSES: u8 = 16;

pub(crate) const DEVICE_ADDRESS: u8 = b'B';
pub(crate) const PURPOSE: &str = "bell";
