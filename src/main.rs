#![cfg_attr(not(test), no_std)]
#![no_main]

mod adc;
mod app;
mod command;
mod led;
mod rs485;

use stm32g0xx_hal as hal;

pub(crate) const RS485_BAUD: u32 = 115200;
pub(crate) const MAX_DETECT_CYCLES: u32 = 8192;
pub(crate) const DEVICE_ADDRESS: protocol::Address = protocol::Address::new(0xb);
