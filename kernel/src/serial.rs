use include::*;

use rustc_unicode;

use cpu;

pub struct Writer;

pub fn setup_serial() {
    unsafe {
        // initialize the serial line
        cpu::write_port_byte(COM1 + 1, 0x00); // disable all interrupts
        cpu::write_port_byte(COM1 + 3, 0x80); // enable DLAB
        cpu::write_port_byte(COM1 + 0, 0x03); // set divisor to 3 (38400 baud)
        cpu::write_port_byte(COM1 + 1, 0x00); // high byte
        cpu::write_port_byte(COM1 + 3, 0x03); // 8 bits, no parity, one stop bit
        cpu::write_port_byte(COM1 + 2, 0xc7); // Enable FIFO, clear them, with 14-byte threshold
        cpu::write_port_byte(COM1 + 4, 0x0b); // IRQ enable, RTS/DSR set
    }
}

fn read_byte() -> u8 {
    unsafe {
        while cpu::read_port_byte(COM1 + 5) & 0x1 == 0 {
            // TODO: implement something better here
        }

        cpu::read_port_byte(COM1)
    }
}

fn write_byte(byte: u8) {
    unsafe {
        while cpu::read_port_byte(COM1 + 5) & 0x20 == 0 {
            // TODO: implement something better here
        }

        cpu::write_port_byte(COM1, byte)
    }
}

pub fn read() -> char {
    let mut buf = [0, 0, 0, 0];
    buf[0] = read_byte();

    let width = rustc_unicode::str::utf8_char_width(buf[0]);

    if width == 1 { return buf[0] as char; }
    if width == 0 { return rustc_unicode::char::REPLACEMENT_CHARACTER; }

    let mut start = 1;
    while start < width {
        buf[start] = read_byte();
    }

    str::from_utf8(&buf[..width]).ok().and_then(|s| s.chars().next())
        .unwrap_or(rustc_unicode::char::REPLACEMENT_CHARACTER)
}

pub fn write(ch: char) {
    for byte in ch.encode_utf8() {
        write_byte(byte);
    }
}

impl Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            write_byte(byte);
        }

        Ok(())
    }
}

impl Writer {
    pub const fn new() -> Writer {
        Writer
    }
}

#[cfg(not(test))]
pub fn reserve_log(message: &Display) {
    static mut WRITER: Writer = Writer::new();

    unsafe {
        let _ = writeln!(WRITER, "{}", message);
    }
}
