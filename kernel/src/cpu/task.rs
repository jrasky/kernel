use std::fmt;

use alloc::arc::Arc;

use spin::RwLock;

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
    Spawn {
        // relevant stack frame and execution location
        entry: u64,
        stack: u64,

        // the arguments we want to pass
        arguments: SpawnArgs
    }
}

#[derive(Debug, Clone)]
pub struct SpawnArgs {
    task: *mut RwLock<TaskInner>,
    previous: *mut RwLock<TaskInner>,
    entry: extern fn(current: Task) -> !
}

#[derive(Debug)]
pub struct Task {
    inner: Arc<RwLock<TaskInner>>,
    previous: Handle
}

#[derive(Debug)]
pub struct Handle {
    inner: Arc<RwLock<TaskInner>>
}

struct TaskInner {
    context: Context,
    entry: extern fn(current: Task) -> !,
    stack: Stack
}

impl fmt::Debug for TaskInner {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "TaskInner {{ context: {:?}, entry: 0x{:x}, stack: {:?} }}",
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
    } else if let &Context::Spawn { ref entry, ref stack, ref arguments } = context {
        unsafe {
            // clobbers is ommitted below to avoid generating code to save registers that we already saved
            asm!(concat!(
                "mov rdi, $0;",
                "mov rsi, $1;",
                "push 0x0;", // simulate a function call
                "jmp $2"
            ) :: "m"(arguments), "*m"(stack), "*m"(entry)
                 :: "intel", "volatile");
        }
    } else {
        panic!("load_context called with non-kernel task!");
    }

    unreachable!("returned from context switch");
}

extern fn empty_entry(_: Task) -> ! {
    unreachable!("Empty entry called");
}

unsafe extern fn spawn_entry(args: *const SpawnArgs) {
    let args = args.as_ref().unwrap();

    let task = Task {
        inner: Arc::from_raw(args.task),
        previous: Handle {
            inner: Arc::from_raw(args.previous)
        }
    };

    (args.entry)(task)
}

impl Task {
    pub unsafe fn empty() -> Task {
        Task {
            inner: Arc::new(RwLock::new(TaskInner::empty())),
            previous: Handle {
                inner: Arc::new(RwLock::new(TaskInner::empty()))
            }
        }
    }

    pub fn spawn(&self, entry: extern fn(task: Task) -> !, stack: Stack) -> Handle {
        let stack_ptr = stack.get_ptr();

        let task = Task {
            inner: Arc::new(RwLock::new(TaskInner {
                context: Context::Empty,
                stack: stack,
                entry: entry
            })),
            previous: Handle {
                inner: self.inner.clone()
            }
        };

        let arguments = SpawnArgs {
            task: Arc::into_raw(task.inner.clone()),
            previous: Arc::into_raw(task.previous.inner.clone()),
            entry: entry
        };

        let context = Context::Spawn {
            entry: spawn_entry as u64,
            stack: stack_ptr as u64,
            arguments: arguments
        };

        task.inner.write().context = context;

        Handle {
            inner: task.inner
            // previous is saved in the context of the Handle, to be used on spawn
        }
    }

    pub fn switch(&mut self, into: Handle) -> Handle {
        self.inner.write().switch(&*into.inner.read());

        into
    }
}

impl TaskInner {
    unsafe fn empty() -> TaskInner {
        TaskInner {
            context: Context::Empty,
            entry: empty_entry,
            stack: Stack::empty()
        }
    }

    fn switch(&mut self, into: &TaskInner) {
        // save our context and execute the other task

        match self.context {
            Context::Empty | Context::Spawn { .. } => {
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
            _ => {}
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
