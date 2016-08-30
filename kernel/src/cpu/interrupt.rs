use std::ptr;

#[allow(dead_code)]
// may be used more later
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
    ss: u64,
}

#[no_mangle]
pub unsafe extern "C" fn interrupt_breakpoint(context: *const Context) {
    let context = ptr::read(context);

    debug!("Breakpoint at 0x{:x}", context.rip);
}

#[no_mangle]
pub unsafe extern "C" fn interrupt_general_protection_fault(context: *const Context) {
    let context = ptr::read(context);

    panic!("General protection fault at 0x{:x}, error 0x{:x}",
           context.rip,
           context.error_code);
}

#[no_mangle]
pub unsafe extern "C" fn interrupt_page_fault(context: *const Context) {
    let context = ptr::read(context);

    let error = if (context.error_code & 1) != 0 {
        if (context.error_code & (1 << 3)) != 0 {
            "reserved bit set"
        } else if (context.error_code & (1 << 5)) != 0 {
            "protection key error"
        } else if (context.error_code & (1 << 15)) != 0 {
            "SGX error"
        } else {
            "other error"
        }
    } else {
        "non-present"
    };

    let access_level = if (context.error_code & (1 << 2)) != 0 {
        "user"
    } else {
        "supervisor"
    };

    let access_type = if (context.error_code & (1 << 4)) != 0 {
        "instruction fetch"
    } else {
        if (context.error_code & (1 << 1)) != 0 {
            "data write"
        } else {
            "data read"
        }
    };

    panic!("Page fault at 0x{:x}: {} on {}-level {}", context.rip, error, access_level, access_type);
}

#[no_mangle]
pub unsafe extern "C" fn early_interrupt_breakpoint(context: *const Context) {
    let context = ptr::read(context);

    debug!("Breakpoint at 0x{:x}", context.rip);
}

#[no_mangle]
pub unsafe extern "C" fn early_interrupt_general_protection_fault(context: *const Context) {
    let context = ptr::read(context);

    panic!("General protection fault at 0x{:x}, error 0x{:x}",
           context.rip,
           context.error_code);
}

#[no_mangle]
pub unsafe extern "C" fn early_interrupt_page_fault(context: *const Context) {
    let context = ptr::read(context);

    let error = if (context.error_code & 1) != 0 {
        if (context.error_code & (1 << 3)) != 0 {
            "reserved bit set"
        } else if (context.error_code & (1 << 5)) != 0 {
            "protection key error"
        } else if (context.error_code & (1 << 15)) != 0 {
            "SGX error"
        } else {
            "other error"
        }
    } else {
        "non-present"
    };

    let access_level = if (context.error_code & (1 << 2)) != 0 {
        "user"
    } else {
        "supervisor"
    };

    let access_type = if (context.error_code & (1 << 4)) != 0 {
        "instruction fetch"
    } else {
        if (context.error_code & (1 << 1)) != 0 {
            "data write"
        } else {
            "data read"
        }
    };

    panic!("Page fault at 0x{:x}: {} on {}-level {}", context.rip, error, access_level, access_type);
}
