#[allow(dead_code)] // may be used more later
#[repr(C, packed)]
pub struct Context {
    rax: u64,
    rbx: u64,
    rcx: u64,
    rdx: u64,
    rbp: u64,
    rsi: u64,
    rdi: u64,
    r8: u64,
    r9: u64,
    r10: u64,
    r11: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,

    // begin interrupt info
    error_code: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64
}

#[no_mangle]
pub unsafe extern "C" fn interrupt_breakpoint(context: *const Context) {
    let context = context.as_ref().unwrap();

    debug!("Breakpoint at 0x{:x}", context.rip);
}

#[no_mangle]
pub unsafe extern "C" fn interrupt_general_protection_fault(context: *const Context) {
    let context = context.as_ref().unwrap();

    panic!("General protection fault at 0x{:x}, error 0x{:x}", context.rip, context.error_code);
}
