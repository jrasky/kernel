#[cfg(not(test))]
pub fn read_msr(id: u32) -> u64 {
    let low: u32;
    let high: u32;

    unsafe {
        asm!("rdmsr" : "={eax}"(low), "={edx}"(high) : "{ecx}"(id) :: "intel");
    }

    ((high as u64) << 32) + (low as u64)
}

#[cfg(not(test))]
pub fn write_msr(id: u32, value: u64) {
    unsafe {
        asm!("wrmsr" :: "{eax}"((value % (::core::u32::MAX as u64)) as u32),
             "{edx}"((value >> 32) as usize), "{ecx}"(id) :: "intel", "volatile");
    }
}

#[cfg(test)]
pub fn read_msr(_: u32) -> u64 {
    0
}

#[cfg(test)]
pub fn write_msr(_: u32, _: u64) {
    // nothing
}

pub fn read_port_byte(port: u16) -> u8 {
    let byte: u8;

    unsafe {
        asm!("in al, dx" : "={al}"(byte) : "{dx}"(port) :: "intel");
    }

    byte
}

pub fn write_port_byte(port: u16, byte: u8) {
    unsafe {
        asm!("out dx, al" :: "{al}"(byte), "{dx}"(port) :: "intel", "volatile");
    }
}

pub fn random() -> u64 {
    unsafe {
        let num: u64;

        asm!("rdrand $0" : "=r"(num) ::: "intel");

        num
    }
}
