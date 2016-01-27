use cpu::stack::Stack;

extern "C" {
    static _fxsave_task: u8;
}

#[repr(u8)]
enum PrivilegeLevel {
    CORE = 0,       // privileged instructions
    DRIVER = 1,     // permissioned-mapped i/o
    EXECUTIVE = 2,  // identity page-map
    USER = 3        // isolated
}

struct Context {
    // FP/MMX/SSE state
    fxsave: [u8; 0x200],

    // GP register state
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

    // execution state
    rflags: u64,
    rip: u64,
    rsp: u64,

    // selectors
    cs: u16,
    ss: u16,
    ds: u16,
    es: u16,
    fs: u16,
    gs: u16
}

struct Task {
    context: Context,
    entry: extern "C" fn(),
    level: PrivilegeLevel,
    stack: Stack
}

impl Default for Context {
    fn default() -> Context {
        Context {
            fxsave: [0; 0x200],
            rax: 0,
            rbx: 0,
            rcx: 0,
            rdx: 0,
            rbp: 0,
            rsi: 0,
            rdi: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rflags: 0,
            rip: 0,
            rsp: 0,
            cs: 0,
            ss: 0,
            ds: 0,
            es: 0,
            fs: 0,
            gs: 0
        }
    }
}

impl Task {
    fn create(level: PrivilegeLevel, entry: extern "C" fn(), stack: Stack) -> Task {
        unsafe {
            // fxsave, use current floating point state in task
            // TODO: actually figure out how to create a clear state and use that
            // instead
            asm!("fxsave $0"
                 :: "i"(_fxsave_task)
                 :: "intel");
        }

        let mut context = Context::default();

        context.rip = entry as u64;
        context.rsp = stack.get_ptr() as u64;

        Task {
            context: context,
            entry: entry,
            level: level,
            stack: stack
        }
    }
}
