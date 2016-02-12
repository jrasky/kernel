pub mod init;
pub mod task;
pub mod stack;
pub mod interrupt;
pub mod syscall;

#[cfg(not(test))]
pub unsafe fn read_msr(id: u32) -> u64 {
    let low: u32;
    let high: u32;

    asm!("rdmsr" : "={eax}"(low), "={edx}"(high) : "{ecx}"(id) :: "intel");

    ((high as u64) << 32) + (low as u64)
}

#[cfg(not(test))]
pub unsafe fn write_msr(id: u32, value: u64) {
    asm!("wrmsr" :: "{eax}"(value), "{edx}"(value >> 32), "{ecx}"(id) ::: "intel");
}

#[cfg(test)]
pub unsafe fn read_msr(_: u32) -> u64 {
    0
}

#[cfg(test)]
pub unsafe fn write_msr(_: u32, _: u64) {
    // nothing
}