use cpu::stack::Stack;

use constants::*;

// things that are clobbered by sysenter:
// RIP, RSP, CS, SS
// RDI, RSI, RDX, RCX

extern "C" {
    fn _sysenter_landing(rip: u64, rsp: u64, cs: u16, ss: u16) -> !;
    fn _sysenter_return(rip: u64, rsp: u64, cs: u16, ss: u16) -> !;
}

pub unsafe fn setup() -> Stack {
    // syscall handler doesn't need a huge stack
    let stack = Stack::create(0x1000);

    // write MSRs
    ::cpu::write_msr(SYSENTER_CS_MSR, CORE_CS as u64);
    ::cpu::write_msr(SYSENTER_EIP_MSR, _sysenter_landing as u64);
    ::cpu::write_msr(SYSENTER_ESP_MSR, stack.get_ptr() as u64);

    // return stack
    stack
}

#[no_mangle]
pub unsafe extern "C" fn sysenter_handler(rip: u64, rsp: u64, cs: u16, ss: u16) -> ! {
    debug!("sysenter_handler reached");

    _sysenter_return(rip, rsp, cs, ss);
}
