//! RS485 arbiter.

use crate::command::Command;
use crate::hal::{
    dma::{self, Channel, Target},
    prelude::*,
    serial::{FullConfig, Rx, Serial, Tx},
    stm32::{TIM17, USART1},
    timer::Timer,
};
use heapless::String;
use protocol::{incoming, Address};
use nb::Error as NbError;

pub struct SendError;

type UART = USART1;
type UARTRX = Rx<UART, FullConfig>;
type UARTTX = Tx<UART, FullConfig>;
type DMA = dma::C1;
type TIMER = Timer<TIM17>;

const SLOTSIZE: u32 = 32; // bytes

pub type BUF = String<64>;

impl From<crate::hal::serial::Error> for SendError {
    fn from(_: crate::hal::serial::Error) -> Self {
        Self
    }
}

#[derive(PartialEq, Eq)]
enum Token {
    Unknown(u32),
    Addr(Address),
    Sending,
}

pub struct Rs485 {
    rx: UARTRX,
    _tx: UARTTX,
    timer: TIMER,
    tx_dma: DMA,
    parser: incoming::Parser,
    bus_busy: bool,
    token: Token,
    alone_cycles: u32,
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
            alone_cycles: 0,
        }
    }

    pub fn interrupt(
        &mut self,
        timer: bool,
        write_fn: impl FnOnce(&mut BUF) -> bool,
    ) -> Option<Command> {
        let rd = self.read(timer);

        if self.is_my_turn() {
            self.transmit(write_fn);
        }

        rd
    }

    fn read(&mut self, mut timer: bool) -> Option<Command> {
        if timer {
            self.timer.clear_irq();
        }

        loop {
            match self.rx.read() {
                Ok(byte) => {
                    self.bus_busy = true;
                    self.timer.active();
                    timer = false;
                    if let Some(msg) = self.parser.feed(byte as char) {
                        self.parser.reset();
                        if self.token != Token::Sending {
                            self.alone_cycles = 0;
                            self.token = Token::Addr(msg.sender);
                        }
                        if let Some(cmd) = Command::from_rs485(msg) {
                            return Some(cmd);
                        }
                    }
                }

                Err(NbError::WouldBlock) => {
                    break;
                }

                Err(NbError::Other(_)) => {
                    // Bad news, perhaps we have a bus collision.
                    // First, stop any ongoing transmission - NOW.
                    self.tx_dma.disable();
                    // Errors leave a corrupted value in the RX register and leave
                    // the interrupt flag in active state, so read-out and ignore
                    // any leftovers we have.
                    let _ = self.rx.read();
                    // And initiate bus re-negotiating.
                    self.parser.reset();
                    self.bus_busy = false;
                    self.token = Token::Unknown(0);
                    self.alone_cycles = 0;
                    self.timer.inactive();
                    return None;
                }
            }
        }

        if self.rx.is_idle() {
            self.parser.reset();
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
                        (
                            Token::Addr(Address::first()),
                            Some(Command::no_connection()),
                        )
                    }
                }
                Token::Sending => (Token::Addr(crate::DEVICE_ADDRESS.next()), None),
                Token::Addr(a) => {
                    if self.alone_cycles < crate::MAX_ALONE_CYCLES + 1 {
                        self.alone_cycles += 1;
                    }
                    let res = if self.alone_cycles == crate::MAX_ALONE_CYCLES {
                        Some(Command::no_connection())
                    } else {
                        None
                    };
                    (Token::Addr(a.next()), res)
                }
            };
            self.token = q.0;
            if let Some(x) = q.1 {
                return Some(x);
            }
        }

        None
    }

    fn is_my_turn(&self) -> bool {
        !self.bus_busy && self.token == Token::Addr(crate::DEVICE_ADDRESS)
    }

    fn transmit(&mut self, datagen: impl FnOnce(&mut BUF) -> bool) {
        static mut BUF: BUF = String::new();
        unsafe {
            BUF.clear();
            self.tx_dma.disable();
            if datagen(&mut BUF) {
                self.token = Token::Sending;
                self.tx_dma.set_memory_address(BUF.as_ptr() as u32, true);
                self.tx_dma.set_transfer_length(BUF.len() as u16);
                self.tx_dma.enable();
            }
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
