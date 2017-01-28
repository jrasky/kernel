use std::fmt;

use cpu::stack::Stack;

#[derive(Debug, Clone)]
pub enum Context {
    Empty,
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
    Call {
        // relevant stack frame and execution location
        entry: u64,
        stack: u64,

        // simplified calling convention only permits one integer argument
        argument: u64,
    }
}

pub struct Task {
    context: Context,
    entry: extern fn(previous: &mut Task) -> !,
    stack: Stack
}

impl fmt::Debug for Task {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "Task {{ context: {:?}, entry: 0x{:x}, stack: {:?} }}",
               self.context, self.entry as u64, self.stack)
    }
}

#[no_mangle]
pub extern "C" fn load_context(context: &Context) -> ! {
    if let &Context::Kernel { ref rip, ref rbx, ref rsp, ref rbp, ref r12, ref r13, ref r14, ref r15 } = context {
        unsafe {
            // clobbers is ommitted below to avoid generating code to save registers that we already saved
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
                 :: "intel", "volatile");
        }
    } else if let &Context::Call { ref entry, ref stack, ref argument } = context {
        unsafe {
            // clobbers is ommitted below to avoid generating code to save registers that we already saved
            asm!(concat!(
                "mov rdi, $0;",
                "mov rsp, $1;",
                "push 0x0;", // simulate a function call
                "jmp $2"
            ) :: "*m"(argument), "*m"(stack), "*m"(entry)
                 :: "intel", "volatile");
        }
    } else {
        panic!("load_context called with non-kernel task!");
    }

    unreachable!("returned from context switch");
}

extern fn empty_entry(_: &mut Task) -> ! {
    unreachable!("Empty entry called");
}

impl Task {
    pub unsafe fn empty() -> Task {
        Task {
            context: Context::Empty,
            entry: empty_entry,
            stack: Stack::empty()
        }
    }

    pub fn spawn(&mut self, entry: extern fn(previous: &mut Task) -> !, stack: Stack) -> Task {
        let task = Task {
            context: Context::Call {
                entry: entry as u64,
                stack: stack.get_ptr() as u64,
                argument: self as *mut _ as u64,
            },
            stack: stack,
            entry: entry
        };

        self.switch(task)
    }

    pub fn switch(&mut self, into: Task) -> Task {
        // save our context and execute the other task

        if let Context::Empty = self.context {
            self.context = Context::Kernel {
                rip: 0,
                rbx: 0,
                rsp: 0,
                rbp: 0,
                r12: 0,
                r13: 0,
                r14: 0,
                r15: 0
            };
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

        into
    }
}
