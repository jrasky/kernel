use std::cell::{RefCell, RefMut};

use std::fmt;
use std::mem;
use std::ptr;

use alloc::rc::Rc;

use kernel_std::cpu::stack::Stack;

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone, Copy)]
pub struct SpawnArgs {
    task: *mut RefCell<TaskInner>,
    previous: *mut RefCell<TaskInner>,
    entry: extern fn(current: Task) -> !
}

#[derive(Debug)]
pub struct LoadHook<'a> {
    outer: RefMut<'a, TaskInner>,
    inner: RefMut<'a, TaskInner>
}

#[derive(Debug)]
pub struct Task {
    inner: Rc<RefCell<TaskInner>>,
    previous: Handle
}

#[derive(Debug)]
pub struct Handle {
    inner: Rc<RefCell<TaskInner>>
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
pub unsafe extern "C" fn load_context(hook: *mut LoadHook) -> ! {
    // copy the info we need out of the hook object
    let hook = ptr::read(hook);

    // copy out the context to the stack
    let context = hook.inner.context;

    // unlock the locks held while saving the context
    mem::drop(hook);

    if let Context::Kernel { ref rip, ref rbx, ref rsp, ref rbp, ref r12, ref r13, ref r14, ref r15 } = context {
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
    } else if let Context::Spawn { ref entry, ref stack, ref arguments } = context {
        // clobbers is ommitted below to avoid generating code to save registers that we already saved
        asm!(concat!(
            "mov rdi, $0;",
            "mov rsi, $1;",
            "push 0x0;", // simulate a function call
            "jmp $2"
        ) :: "m"(arguments), "*m"(stack), "*m"(entry)
             :: "intel", "volatile");
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
        inner: Rc::from_raw(args.task),
        previous: Handle {
            inner: Rc::from_raw(args.previous)
        }
    };

    (args.entry)(task)
}

fn switch(mut hook: LoadHook) {
    // save our context and execute the other task
    match hook.outer.context {
        Context::Empty | Context::Spawn { .. } => {
            hook.outer.context = Context::Kernel {
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

    unsafe {
        // What we're doing doesn't violate borrowing rules because we only use the
        // mutable references to the context fields before calling load_context.
        let hook_ptr = &mut hook as *mut _;

        if let Context::Kernel { ref mut rip, ref mut rbx, ref mut rsp,
                                 ref mut rbp, ref mut r12, ref mut r13,
                                 ref mut r14, ref mut r15 } = hook.outer.context
        {
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
                 : "m"(hook_ptr) : "rdi" : "intel", "volatile");
        } else {
            unimplemented!();
        }

        // IMPORTANT: hook has been "moved" at this point, so forget the value
        // to avoid running the destructor twice.
        mem::forget(hook);
    }
}

impl Task {
    pub unsafe fn empty() -> Task {
        Task {
            inner: Rc::new(RefCell::new(TaskInner::empty())),
            previous: Handle {
                inner: Rc::new(RefCell::new(TaskInner::empty()))
            }
        }
    }

    pub fn spawn(&self, entry: extern fn(task: Task) -> !, stack: Stack) -> Handle {
        let stack_ptr = stack.get_ptr();

        let task = Task {
            inner: Rc::new(RefCell::new(TaskInner {
                context: Context::Empty,
                stack: stack,
                entry: entry
            })),
            previous: Handle {
                inner: self.inner.clone()
            }
        };

        let arguments = SpawnArgs {
            task: Rc::into_raw(task.inner.clone()),
            previous: Rc::into_raw(task.previous.inner.clone()),
            entry: entry
        };

        let context = Context::Spawn {
            entry: spawn_entry as u64,
            stack: stack_ptr as u64,
            arguments: arguments
        };

        task.inner.borrow_mut().context = context;

        Handle {
            inner: task.inner
            // previous is saved in the context of the Handle, to be used on spawn
        }
    }

    pub fn yield_back(&mut self) {
        // these locks need to be unlocked after the context switch
        let hook = LoadHook {
            outer: self.inner.borrow_mut(),
            inner: self.previous.inner.borrow_mut()
        };

        switch(hook);
    }

    pub fn switch(&mut self, into: &mut Handle) {
        // these locks need to be unlocked after the context switch
        let hook = LoadHook {
            outer: self.inner.borrow_mut(),
            inner: into.inner.borrow_mut()
        };

        switch(hook);
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
}
