#[cfg(not(test))]
pub unsafe fn read_msr(id: u32) -> u64 {
    let low: u32;
    let high: u32;

    asm!("rdmsr" : "={eax}"(low), "={edx}"(high) : "{ecx}"(id) :: "intel");

    ((high as u64) << 32) + (low as u64)
}

#[cfg(not(test))]
pub unsafe fn write_msr(id: u32, value: u64) {
    asm!("wrmsr" :: "{eax}"((value % (::core::u32::MAX as u64)) as u32), "{edx}"((value >> 32) as usize), "{ecx}"(id) :: "intel");
}

#[cfg(test)]
pub unsafe fn read_msr(_: u32) -> u64 {
    0
}

#[cfg(test)]
pub unsafe fn write_msr(_: u32, _: u64) {
    // nothing
}

pub unsafe fn read_port_byte(port: u16) -> u8 {
    let byte: u8;

    asm!("in al, dx" : "={al}"(byte) : "{dx}"(port) :: "intel");

    byte
}

pub unsafe fn write_port_byte(port: u16, byte: u8) {
    asm!("out dx, al" :: "{al}"(byte), "{dx}"(port) :: "intel");
}

pub fn random() -> u64 {
    unsafe {
        let num: u64;

        asm!("rdrand $0" : "=r"(num) ::: "intel");

        num
    }
}
