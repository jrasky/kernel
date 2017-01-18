#![feature(const_fn)]
#![no_std]
extern crate core as std;
extern crate constants;
extern crate log;

use std::fmt::Write;

use std::fmt;
use std::str;

pub use constants::*;

pub struct ReserveLogger;

pub fn setup_serial() {
    // initialize the serial line
    util::write_port_byte(COM1 + 1, 0x00); // disable all interrupts
    util::write_port_byte(COM1 + 3, 0x80); // enable DLAB
    util::write_port_byte(COM1 + 0, 0x03); // set divisor to 3 (38400 baud)
    util::write_port_byte(COM1 + 1, 0x00); // high byte
    util::write_port_byte(COM1 + 3, 0x03); // 8 bits, no parity, one stop bit
    util::write_port_byte(COM1 + 2, 0xc7); // Enable FIFO, clear them, with 14-byte threshold
    util::write_port_byte(COM1 + 4, 0x0b); // IRQ enable, RTS/DSR set
}

fn read_byte() -> u8 {
    while util::read_port_byte(COM1 + 5) & 0x1 == 0 {
        // TODO: implement something better here
    }

    util::read_port_byte(COM1)
}

fn write_byte(byte: u8) {
    while util::read_port_byte(COM1 + 5) & 0x20 == 0 {
        // TODO: implement something better here
    }

    util::write_port_byte(COM1, byte)
}

pub fn write(buf: &[u8]) -> Result<usize, fmt::Error> {
    for byte in buf.iter() {
        write_byte(*byte);
    }

    Ok(buf.len())
}

pub fn read(buf: &mut [u8]) -> Result<usize, fmt::Error> {
    for i in 0..buf.len() {
        buf[i] = read_byte();
    }

    Ok(buf.len())
}

impl Write for ReserveLogger {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if let Ok(len) = write(s.as_bytes()) {
            if len != s.as_bytes().len() {
                // error if not all bytes were written
                return Err(fmt::Error);
            }
        }

        Ok(())
    }
}

impl ReserveLogger {
    pub const fn new() -> ReserveLogger {
        ReserveLogger
    }
}

impl log::Log for ReserveLogger {
    fn enabled(&self, _: &log::LogMetadata) -> bool {
        true
    }

    fn log(&self, record: &log::LogRecord) {
        static mut LOGGER: ReserveLogger = ReserveLogger::new();

        unsafe {
            let _ = writeln!(
                LOGGER, "{} RESERVE at {}({}): {}", record.target(), 
                record.location().file(), record.location().line(),
                record.args());
        }
    }
}
