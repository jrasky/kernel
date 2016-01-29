use collections::VecDeque;

use core::cell::UnsafeCell;

use core::ptr;

use alloc::boxed::Box;

use spin::Mutex;

use cpu::stack::Stack;

extern "C" {
    static mut _fxsave_task: u8;
    fn _do_execute(regs: *const Regs, busy: *mut u16, core_regs: *mut Regs);
    fn _do_execute_nobranch(regs: *const Regs);
    fn _load_context(regs: *mut Regs);
}

extern "C" fn _dummy_entry() {
    unreachable!("Tried to entry dummy entry");
}

static mut MANAGER: Manager = Manager::new();

pub fn switch_core() {
    unsafe {
        MANAGER.switch_core();
    }
}

#[repr(u8)]
pub enum PrivilegeLevel {
    CORE = 0,       // privileged instructions
    DRIVER = 1,     // permissioned-mapped i/o
    EXECUTIVE = 2,  // identity page-map
    USER = 3        // isolated
}

struct Manager {
    inner: *mut ManagerInner
}

struct ManagerInner {
    core: Task,
    tasks: VecDeque<Task>
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
    busy: u16
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

impl Manager {
    const fn new() -> Manager {
        Manager {
            inner: ptr::null_mut()
        }
    }

    #[inline]
    fn get_inner(&mut self) -> &mut ManagerInner {
        unsafe {
            if let Some(inner) = self.inner.as_mut() {
                inner
            } else {
                let inner = Box::new(ManagerInner::new());
                self.inner = Box::into_raw(inner);
                self.inner.as_mut().unwrap()
            }
        }
    }

    #[inline]
    pub fn switch_task(&mut self, context: &mut Context) {
        self.get_inner().switch_task(context)
    }

    #[inline]
    pub fn switch_core(&mut self) {
        self.get_inner().switch_core()
    }
}

impl ManagerInner {
    fn new() -> ManagerInner {
        ManagerInner {
            core: Task {
                context: Context::empty(),
                entry: _dummy_entry,
                level: PrivilegeLevel::CORE,
                stack: unsafe {Stack::kernel()},
                busy: !0
            },
            tasks: VecDeque::new()
        }
    }

    fn switch_task(&mut self, context: &mut Context) {
        if self.core.busy == 0 {
            panic!("Tried to switch tasks while not in core task");
        }

        unsafe {
            // save our 
            asm!("fxsave $0"
                 : "=*m"(&mut _fxsave_task)
                 ::: "intel");

            ptr::copy(&mut _fxsave_task, self.core.context.fxsave.as_mut_ptr(),
                      self.core.context.fxsave.len());

            ptr::copy(context.fxsave.as_ptr(), &mut _fxsave_task as *mut u8,
                      context.fxsave.len());

            asm!("fxrstor $0"
                 :: "*m"(&_fxsave_task)
                 :: "intel");

            debug!("Executing task");

            _do_execute(&context.regs, &mut self.core.busy, &mut self.core.context.regs);

            debug!("Switched back");
        }
    }

    fn switch_core(&mut self) {
        // switch back to the core task
        if self.core.busy != 0 {
            panic!("Tried to switch back to core task while in core task");
        }

        unsafe {
            ptr::copy(self.core.context.fxsave.as_ptr(), &mut _fxsave_task as *mut u8,
                      self.core.context.fxsave.len());

            asm!("fxrstor $0"
                 :: "*m"(&_fxsave_task)
                 :: "intel");

            debug!("Switching back to core");
            
            _do_execute_nobranch(&self.core.context.regs);

            unreachable!("_do_execute_nobranch() returned");
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
        // create a blank context
        let mut context = Context::empty();

        unsafe {
            // fxsave, use current floating point state in task
            // TODO: generate a compliant FPU state instead of just using the current one
            asm!("fxsave $0"
                 : "=*m"(&mut _fxsave_task)
                 ::: "intel");

            // copy fxsave area
            ptr::copy(&mut _fxsave_task, context.fxsave.as_mut_ptr(),
                      context.fxsave.len());
        }

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
            busy: 0
        }
    }

    pub fn execute(&mut self) {
        // save core task

        if self.busy == 0 {
            // start task
            self.busy = !0;
            unsafe {MANAGER.switch_task(&mut self.context);}
        } else {
            // clean up
            self.busy = 0;
        }
    }
}
