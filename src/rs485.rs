//! RS485 arbiter.

use crate::hal::{
    block,
    prelude::*,
    serial::{FullConfig, Serial, Rx, Tx},
    timer::Timer,
    dma::{self, Channel, Target},
    stm32::{USART1, TIM17},
};
use heapless::String;

pub struct SendError;

type UART = USART1;
type UARTRX = Rx<UART, FullConfig>;
type UARTTX = Tx<UART, FullConfig>;
type DMA = dma::C1;
type TIMER = Timer<TIM17>;

const SLOTSIZE: u32 = 10; // bytes

impl From<crate::hal::serial::Error> for SendError {
    fn from(_: crate::hal::serial::Error) -> Self {
        Self
    }
}

pub struct Rs485Rx {
    uart: UARTRX,
    timer: TIMER, 
    token: Option<u8>,
}

pub struct DataPacket {
    pub voltage: u16,
    pub temperature: i16,
    pub button: Option<crate::adc::Button>,
}

impl Rs485Rx {
    pub fn new(mut uart: UARTRX, mut timer: TIMER) -> Self {
        timer.start((1_000_000 / SLOTSIZE * (10 + 0) / crate::RS485_BAUD).micros());

        uart.listen();
        uart.listen_idle();

        timer.listen();

        Self {
            uart,
            timer,
            token: None,
        }
    }

    pub fn interrupt(&mut self, mut timer: bool) -> Option<u8> {
        while self.uart.is_rxne() {
            if let Ok(byte) = self.uart.read() {
                self.timer.active();
                timer = false;
                return Some(byte); // TODO
            }
        }

        if self.uart.is_idle() {
            self.timer.inactive();
            timer = false;
            self.uart.clear_idle();
        }

        if timer {
            self.timer.clear_irq();    
        }

        None
    }
/*
  fn send_packet_raw(&mut self, data: &[u8]) -> Result<(), SendError> {
        self.timer.active();
        let ret = unimplementedself.uart.send_bytes(data.into_iter().cloned());
        self.timer.inactive();
        ret
    }

    pub fn send_packet(&mut self, packet: DataPacket) -> Result<(), SendError> {
        let mut buf = String::<32>::new();
        write!(
            buf,
            "{}:{}:T{temp:+02},V{volt:04}\n",
            crate::DEVICE_ADDRESS,
            crate::PURPOSE,
            temp = packet.temperature,
            volt = packet.voltage
        )
        .expect("Not enough space in string");
        self.send_packet_raw(buf.as_bytes())
    }
    */
}

pub type BUF = String<64>;

pub struct Rs485Tx {
    tx: UARTTX,
    dma: DMA,
}

impl Rs485Tx {
    pub fn new(mut tx: UARTTX, mut dma: DMA) -> Self {
        unsafe {
            dma.set_direction(dma::Direction::FromMemory);
            dma.set_peripheral_address(&(*UART::ptr()).tdr as *const _ as u32, false);
            dma.select_peripheral(tx.dmamux());
            tx.enable_dma();
        }
        dma.listen(dma::Event::TransferComplete);

        Self {
            tx,
            dma,
        }
    }

    pub fn transmit(&mut self, datagen: impl FnOnce(&mut BUF)) {
        static mut BUF: BUF = String::new(); 
        unsafe {
            BUF.clear();
            let len = datagen(&mut BUF);
            self.dma.disable();
            self.dma.set_memory_address(BUF.as_ptr() as u32, true);
            self.dma.set_transfer_length(BUF.len() as u16);

            self.dma.enable();
        }
    }
}



/*trait UartWithFeedback {
    fn send_bytes(&mut self, bytes: impl IntoIterator<Item = u8>) -> Result<(), SendError>;
}

impl UartWithFeedback for UARTRX {
    fn send_bytes(&mut self, bytes: impl IntoIterator<Item = u8>) -> Result<(), SendError> {
        for byte in bytes {
            block!(self.write(byte))?;
            let feedback = block!(self.read())?;
            if feedback != byte {
                return Err(SendError);
            }
        }
        Ok(())
    }
}*/


trait ActivityTimer {
    fn active(&mut self);
    fn inactive(&mut self);
}

impl ActivityTimer for TIMER {
    fn active(&mut self) {
        self.pause();
        self.reset();
        self.clear_irq();
    }

    fn inactive(&mut self) {
        self.reset();
        self.resume();
        self.clear_irq();
    }
}
