use cpu::stack::Stack;

#[derive(Debug, Clone)]
pub enum Context {
    Kernel {
        // SYSV callee-saved registers
        rip: u64,
        rbx: u64,
        rsp: u64,
        rbp: u64,
        r12: u64,
        r13: u64,
        r14: u64,
        r15: u64
    },
    New {
        // relevant stack frame and execution location
        rip: u64,
        rsp: u64,

        // registers for passing integer arguments
        rdi: u64,
        rsi: u64,
        rdx: u64,
        rcx: u64,
        r8: u64,
        r9: u64,
    }
}

#[derive(Debug)]
pub struct Task {
    context: Context,
    stack: Stack
}

#[no_mangle]
pub extern "C" fn load_context(context: &Context) -> ! {
    if let &Context::Kernel { ref rip, ref rbx, ref rsp, ref rbp, ref r12, ref r13, ref r14, ref r15 } = context {
        unsafe {
            asm!(concat!(
                "mov rbx, $0;",
                "mov rsp, $1;",
                "mov rbp, $2;",
                "mov r12, $3;",
                "mov r13, $4;",
                "mov r14, $5;",
                "mov r15, $6;",
                "jmp $7"
            ) :: "*m"(rbx), "*m"(rsp), "*m"(rbp), "*m"(r12), "*m"(r13), "*m"(r14), "*m"(r15), "*m"(rip)
                 : "rbx", "rbp", "r12", "r13", "r14", "r15" : "intel", "volatile");
        }

        unreachable!("returned from context switch");
    } else if let &Context::New { ref rip, ref rsp, ref rdi, ref rsi, ref rdx, ref rcx, ref r8, ref r9 } = context {
        unsafe {
            asm!(concat!(
                "mov rdi, $0;",
                "mov rsi, $1;",
                "mov rdx, $2;",
                "mov rcx, $3;",
                "mov r8, $4;",
                "mov r9, $5;",
                "mov rsp, $6;",
                "push 0x0;", // simulate a function call
                "jmp $7"
            ) :: "*m"(rdi), "*m"(rsi), "*m"(rdx), "*m"(rcx), "*m"(r8), "*m"(r9), "*m"(rsp), "*m"(rip)
                 : "rdi", "rsi", "rdx", "rcx", "r8", "r9" : "intel", "volatile");
        }

        unreachable!("returned from context switch");
    } else {
        panic!("load_context called with non-kernel task!");
    }
}

impl Default for Context {
    fn default() -> Context {
        Context::Kernel {
            rip: 0,
            rbx: 0,
            rsp: 0,
            rbp: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0
        }
    }
}

impl Task {
    pub fn new(context: Context, stack: Stack) -> Task {
        Task {
            context: context,
            stack: stack
        }
    }

    pub fn switch(&mut self, into: &Task) {
        // save our context and execute the other task

        if let Context::New { .. } = self.context {
            // replace our context with a standard one if we're a new task
            self.context = Context::default();
        }

        if let Context::Kernel { ref mut rip, ref mut rbx, ref mut rsp,
                                 ref mut rbp, ref mut r12, ref mut r13,
                                 ref mut r14, ref mut r15 } = self.context
        {
            unsafe {
                asm!(concat!(
                    "mov $0, rbx;",
                    "mov $1, rsp;",
                    "mov $2, rbp;",
                    "mov $3, r12;",
                    "mov $4, r13;",
                    "mov $5, r14;",
                    "mov $6, r15;",
                    "lea rdi, .continue;",
                    "mov $7, rdi;",
                    "mov rdi, $8;",
                    "call load_context;",
                    ".continue: nop"
                ) : "=*m"(rbx), "=*m"(rsp), "=*m"(rbp), "=*m"(r12), "=*m"(r13), "=*m"(r14),
                     "=*m"(r15), "=*m"(rip)
                     : "m"(&into.context) : "rdi" : "intel", "volatile");
            }
        } else {
            unimplemented!();
        }
    }
}
