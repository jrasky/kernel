use collections::VecDeque;

use core::ptr;

use spin::Mutex;

use cpu::stack::Stack;

extern "C" {
    static mut _fxsave_task: u8;
    fn _do_execute(regs: *const Regs);
}

static MANAGER: Mutex<Manager> = Mutex::new(Manager {
    core: None,
    tasks: None
});

#[repr(u8)]
pub enum PrivilegeLevel {
    CORE = 0,       // privileged instructions
    DRIVER = 1,     // permissioned-mapped i/o
    EXECUTIVE = 2,  // identity page-map
    USER = 3        // isolated
}

struct Manager {
    core: Option<Task>,
    tasks: Option<VecDeque<Task>>
}

#[repr(packed)]
struct Regs {
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

struct Context {
    // FP/MMX/SSE state
    fxsave: [u8; 0x200],
    regs: Regs
}

pub struct Task {
    context: Context,
    entry: extern "C" fn(),
    level: PrivilegeLevel,
    stack: Stack,
    busy: bool
}

impl Regs {
    pub const fn empty() -> Regs {
        Regs {
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

impl Context {
    pub const fn empty() -> Context {
        Context {
            fxsave: [0; 0x200],
            regs: Regs::empty()
        }
    }
}

impl Task {
    pub fn create(level: PrivilegeLevel, entry: extern "C" fn(), stack: Stack) -> Task {
        unsafe {
            // fxsave, use current floating point state in task
            // TODO: generate a compliant FPU state instead of just using the current one
            asm!("fxsave $0"
                 : "=*m"(&_fxsave_task)
                 ::: "intel");
        }

        // create a blank context
        let mut context = Context::empty();

        // set the initial parameters
        context.regs.rip = entry as u64;
        // correctly align the stack. Assumes the stack pointer is 16-bytes aligned
        context.regs.rsp = stack.get_ptr() as u64 - 0x08;

        // only use kernel segments for now
        context.regs.cs = 0x01 << 3; // second segment, GDT, RPL 0
        context.regs.ss = 0x02 << 3; // third segment, GDT, RPL 0
        context.regs.ds = 0x02 << 3;
        context.regs.es = 0x02 << 3;
        context.regs.fs = 0x02 << 3;
        context.regs.gs = 0x02 << 3;

        Task {
            context: context,
            entry: entry,
            level: level,
            stack: stack,
            busy: false
        }
    }

    pub fn execute(&mut self) {
        // save core task

        if !self.busy {
            // start task
            unsafe {self.execute_inner();}
        } else {
            // clean up
            self.busy = false;
        }
    }

    unsafe fn execute_inner(&mut self) -> ! {
        // copy fxsave for task to the fxsave area
        ptr::copy(self.context.fxsave.as_ptr(), &mut _fxsave_task as *mut u8,
                  self.context.fxsave.len());

        // fxrstor
        asm!("fxrstor $0"
             :: "i"(_fxsave_task)
             :: "intel");

        // restore register values and jump to proceedure
        _do_execute(&self.context.regs);

        unreachable!("_do_execute() returned");
    }
}
