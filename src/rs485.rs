//! RS485 arbiter.

use crate::hal::{
    prelude::*,
    serial::{FullConfig, Serial, Rx, Tx},
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

pub type BUF = String<32>;

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

pub struct Rs485 {
    rx: UARTRX,
    _tx: UARTTX,
    timer: TIMER, 
    tx_dma: DMA,
    parser: incoming::Parser,
    bus_busy: bool,
    token: Token,
}

impl Rs485 {
    pub fn new(uart: Serial<UART, FullConfig>, mut timer: TIMER, mut tx_dma: DMA) -> Self {
        let (mut tx, mut rx) = uart.split();

        unsafe {
            tx_dma.set_direction(dma::Direction::FromMemory);
            tx_dma.set_peripheral_address(&(*UART::ptr()).tdr as *const _ as u32, false);
            tx_dma.select_peripheral(tx.dmamux());
            tx.enable_dma();
        }
        tx_dma.listen(dma::Event::TransferComplete);

        timer.start((1_000_000_u32 * (10 + 0) * SLOTSIZE / crate::RS485_BAUD).micros());

        rx.listen();
        rx.listen_idle();

        timer.listen();

        Self {
            rx,
            _tx: tx,
            parser: incoming::Parser::new(),
            timer,
            tx_dma,
            bus_busy: true,
            token: Token::Unknown(0),
        }
    }

    pub fn is_my_turn(&self) -> bool {
        ! self.bus_busy && self.token == Token::Addr(crate::DEVICE_ADDRESS)
    }

    pub fn interrupt(&mut self, mut timer: bool) -> Option<u8> {
        if timer {
            self.timer.clear_irq();
        }

        while self.rx.is_rxne() {
            self.bus_busy = true;
            if let Ok(byte) = self.rx.read() {
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

        if self.rx.is_idle() {
            self.bus_busy = false;
            self.timer.inactive();
            timer = false;
            self.rx.clear_idle();
        }

        if timer {
            let q = match self.token {
                Token::Unknown(n) => {
                    if n < crate::MAX_DETECT_CYCLES {
                        (Token::Unknown(n + 1), None)
                    } else {
                        (Token::Addr(Address::first()), Some(b'R'))
                    }
                }
                Token::Addr(a) => (Token::Addr(a.next()), None),
            };
            self.token = q.0;
            if let Some(x) = q.1 {
                return Some(x);
            }
        }

        None
    }

    pub fn transmit(&mut self, datagen: impl FnOnce(&mut BUF)) {
        static mut BUF: BUF = String::new(); 
        unsafe {
            BUF.clear();
            datagen(&mut BUF);
            self.tx_dma.disable();
            self.tx_dma.set_memory_address(BUF.as_ptr() as u32, true);
            self.tx_dma.set_transfer_length(BUF.len() as u16);

            self.tx_dma.enable();
        }
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
