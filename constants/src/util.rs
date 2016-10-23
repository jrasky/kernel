#[cfg(not(test))]
pub fn read_msr(id: u32) -> u64 {
    let low: u32;
    let high: u32;

    unsafe {
        asm!("rdmsr" : "={eax}"(low), "={edx}"(high) : "{ecx}"(id) :: "intel");
    }

    ((high as u64) << 32) | (low as u64)
}

#[cfg(not(test))]
pub fn write_msr(id: u32, value: u64) {
    unsafe {
        asm!("wrmsr" :: "{eax}"((value & (::core::u32::MAX as u64)) as u32),
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

/// 64 bit remainder on 32 bit arch
#[no_mangle]
#[cfg(target_pointer_width = "32")]
pub extern "C" fn __umoddi3(mut a: u64, mut b: u64) -> u64 {
    let mut hig: u64 = a >> 32; // The first 32 bits of a
    let mut d: u64 = 1;

    if hig >= b {
        hig /= b;
        a -= (hig * b) << 32;
    }

    while b > 0 && b < a {
        b <<= 1;
        d <<= 1;
    }

    loop {
        if a >= b {
            a -= b;
        }
        b >>= 1;
        d >>= 1;

        if d == 0 {
            break;
        }
    }

    a
}

/// 64 bit division on 32 bit arch
#[no_mangle]
#[cfg(target_pointer_width = "32")]
pub extern "C" fn __udivdi3(mut a: u64, mut b: u64) -> u64 {
    let mut res: u64 = 0;
    let mut hig: u64 = a >> 32; // The first 32 bits of a
    let mut d: u64 = 1u64;

    if hig >= b {
        hig /= b;
        res = hig << 32;
        a -= (hig * b) << 32;
    }

    while b > 0 && b < a {
        b <<= 1;
        d <<= 1;
    }

    loop {
        if a >= b {
            a -= b;
            res += d;
        }
        b >>= 1;
        d >>= 1;

        if d == 0 {
            break;
        }
    }

    res
}

#[no_mangle]
#[cfg(target_pointer_width = "32")]
/// 64 bit division and rem on 32 bit arch
pub extern "C" fn __udivremi3(mut a: u64, mut b: u64) -> (u64, u64) {
    let mut res: u64 = 0;
    let mut hig: u64 = a >> 32; // The first 32 bits of a
    let mut d: u64 = 1;

    if hig >= b {
        hig /= b;
        res = hig << 32;
        a -= (hig * b) << 32;
    }

    while b > 0 && b < a {
        b <<= 1;
        d <<= 1;
    }

    loop {
        if a >= b {
            a -= b;
            res += d;
        }
        b >>= 1;
        d >>= 1;

        if d == 0 {
            break;
        }
    }

    (res, a)
}
