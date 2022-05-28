//! RS485 arbiter.

use crate::hal::block;
use embedded_hal::serial::{Read, Write};

pub struct SendError;

pub struct Rs485<UART> {
    uart: UART,
}

impl<UART> Rs485<UART>
where
    UART: Read<u8> + Write<u8>,
    SendError: From<<UART as Write<u8>>::Error>,
    SendError: From<<UART as Read<u8>>::Error>,
{
    pub fn send_packet(&mut self, data: &[u8]) -> Result<(), SendError> {
        for byte in data {
            block!(self.uart.write(*byte))?;
            let feedback = block!(self.uart.read())?;
            if feedback != *byte {
                return Err(SendError);
            }
        }
        Ok(())
    }
}
