#[derive(Debug, Clone)]
enum Context {
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
    }
}

#[derive(Debug)]
struct Task {
    context: Context,
    handler: Fn(&Context) -> !
}

pub fn kernel_handler(context: &Context) -> ! {
    if let Context::Kernel { rip, rbx, rsp, rbp, r12, r13, r14, r15 } = context {
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
                 : "rbx", "rsp", "rbp", "r12", "r13", "r14", "r15" : "intel", "volatile");
        }

        unreachable!("returned from context switch");
    } else {
        panic!("kernel_execute called with non-kernel task!");
    }
}

unsafe extern "C" fn use_handler(task: &Task) {
    task.handler(&task.context)
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
    pub fn new(context: Context, handler: Fn(&Context) -> !) {
        Task {
            context: context,
            handler: handler
        }
    }

    pub fn switch(&mut self, into: &Task) {
        // save our context and execute the other task

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
                    "lea $7, .continue;",
                    "mov rdi, $8;",
                    "call use_handler;",
                    ".continue: nop"
                ) : "*m"(rbx), "*m"(rsp), "*m"(rbp), "*m"(r12), "*m"(r13), "*m"(r14),
                     "*m"(r15), "*m"(rip)
                     : "m"(self) : "rdi" : "intel", "volatile");
            }
        } else {
            unimplemented!();
        }
    }
}
