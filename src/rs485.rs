//! RS485 arbiter.

use crate::hal::{
    prelude::*,
    serial::{FullConfig, Rx, Tx},
    timer::Timer,
    dma::{self, Channel, Target},
    stm32::{USART1, TIM17},
};
use heapless::String;
use protocol::{Address, incoming};

pub struct SendError;

type UART = USART1;
type UARTRX = Rx<UART, FullConfig>;
type UARTTX = Tx<UART, FullConfig>;
type DMA = dma::C1;
type TIMER = Timer<TIM17>;

const SLOTSIZE: u32 = 32; // bytes

impl From<crate::hal::serial::Error> for SendError {
    fn from(_: crate::hal::serial::Error) -> Self {
        Self
    }
}

#[derive(PartialEq, Eq)]
enum Token {
    Unknown(u32),
    Addr(Address),
}

pub struct Rs485Rx {
    uart: UARTRX,
    timer: TIMER, 
    parser: incoming::Parser,
    token: Token,
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
            parser: incoming::Parser::new(),
            token: Token::Unknown(0),
        }
    }

    pub fn is_my_turn(&self) -> bool {
        self.token == Token::Addr(crate::DEVICE_ADDRESS)
    }

    pub fn interrupt(&mut self, mut timer: bool) -> Option<u8> {
        while self.uart.is_rxne() {
            if let Ok(byte) = self.uart.read() {
                self.timer.active();
                timer = false;
                if let Some(msg) = self.parser.feed(byte as char) {
                    self.token = Token::Addr(msg.sender);
                    if let Some(cmd) = msg.command {
                        return Some(cmd)
                    }
                }
            }
        }

        if self.uart.is_idle() {
            self.timer.inactive();
            timer = false;
            self.uart.clear_idle();
        }

        if timer {
            self.timer.clear_irq();
            self.token = match self.token {
                Token::Unknown(n) => {
                    if n < crate::MAX_DETECT_CYCLES {
                        Token::Unknown(n + 1)
                    } else {
                        Token::Addr(Address::first())
                    }
                }
                Token::Addr(a) => Token::Addr(a.next()),
            }
        }

        None
    }
}

pub type BUF = String<32>;

pub struct Rs485Tx {
    _tx: UARTTX,
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
            _tx: tx,
            dma,
        }
    }

    pub fn transmit(&mut self, datagen: impl FnOnce(&mut BUF)) {
        static mut BUF: BUF = String::new(); 
        unsafe {
            BUF.clear();
            datagen(&mut BUF);
            self.dma.disable();
            self.dma.set_memory_address(BUF.as_ptr() as u32, true);
            self.dma.set_transfer_length(BUF.len() as u16);

            self.dma.enable();
        }
    }

    pub fn is_idle(&self) -> bool {
        ! self.dma.is_enabled() // FIXME this is incorrect
    }
}

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
