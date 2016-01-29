use collections::VecDeque;

use core::ptr;

use alloc::boxed::Box;

use cpu::stack::Stack;

extern "C" {
    static mut _fxsave_task: u8;
    fn _do_execute(regs: *const Regs, busy: *mut u16, core_regs: *mut Regs);
    fn _do_execute_nobranch(regs: *const Regs);
    fn _load_context(regs: *mut Regs);
}

extern "C" fn _dummy_entry() -> ! {
    unreachable!("Tried to entry dummy entry");
}

static mut MANAGER: Manager = Manager::new();

pub fn run_next() -> bool {
    unsafe { MANAGER.run_next() }
}

pub fn exit() -> ! {
    unsafe { MANAGER.exit() }
}

pub fn add(task: Task) {
    unsafe { MANAGER.add(task) }
}

#[allow(dead_code)] // may be used in the future
pub fn switch_task(task: Task) -> Task {
    unsafe { MANAGER.switch_task(task) }
}

pub fn switch_core() {
    unsafe { MANAGER.switch_core() }
}

#[repr(u8)]
pub enum PrivilegeLevel {
    CORE = 0, // privileged instructions
    #[allow(dead_code)]
    DRIVER = 1, // permissioned-mapped i/o
    #[allow(dead_code)]
    EXECUTIVE = 2, // identity page-map
    #[allow(dead_code)]
    USER = 3, // isolated
}

struct Manager {
    inner: *mut ManagerInner,
}

struct ManagerInner {
    core: Task,
    tasks: VecDeque<Task>,
    current: Option<Task>,
}

#[repr(C, packed)]
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
    gs: u16,
}

struct Context {
    // FP/MMX/SSE state
    fxsave: [u8; 0x200],
    regs: Regs,
}

pub struct Task {
    inner: Box<TaskInner>,
}

struct TaskInner {
    context: Context,
    #[allow(dead_code)]
    entry: extern "C" fn() -> !,
    #[allow(dead_code)]
    level: PrivilegeLevel,
    #[allow(dead_code)]
    stack: Stack,
    busy: u16,
    done: bool,
}

impl Task {
    #[inline]
    pub fn create(level: PrivilegeLevel, entry: extern "C" fn() -> !, stack: Stack) -> Task {
        Task { inner: Box::new(TaskInner::create(level, entry, stack)) }
    }

    #[inline]
    pub fn set_done(&mut self) {
        self.inner.set_done()
    }

    #[inline]
    pub fn is_done(&mut self) -> bool {
        self.inner.is_done()
    }
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
            gs: 0,
        }
    }
}

impl Manager {
    const fn new() -> Manager {
        Manager { inner: ptr::null_mut() }
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
    pub fn run_next(&mut self) -> bool {
        self.get_inner().run_next()
    }

    #[inline]
    pub fn add(&mut self, task: Task) {
        self.get_inner().add(task)
    }

    #[inline]
    pub fn exit(&mut self) -> ! {
        self.get_inner().exit()
    }

    #[inline]
    pub fn switch_task(&mut self, task: Task) -> Task {
        self.get_inner().switch_task(task)
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
                inner: Box::new(TaskInner {
                    context: Context::empty(),
                    entry: _dummy_entry,
                    level: PrivilegeLevel::CORE,
                    stack: unsafe { Stack::kernel() },
                    busy: !0,
                    done: false,
                }),
            },
            tasks: VecDeque::new(),
            current: None,
        }
    }

    #[inline]
    fn in_core(&self) -> bool {
        self.core.inner.busy != 0
    }

    fn add(&mut self, task: Task) {
        self.tasks.push_back(task);
    }

    fn run_next(&mut self) -> bool {
        // get the next task
        if let Some(mut task) = self.tasks.pop_front() {

            // switch to it
            task = self.switch_task(task);

            // add it if it isn't done
            if !task.is_done() {
                self.tasks.push_back(task);
            }

            // done
            true
        } else {
            // no tasks
            false
        }
    }

    fn exit(&mut self) -> ! {
        if self.in_core() {
            panic!("Cannot exit core task");
        }

        if let Some(mut task) = self.current.take() {
            debug!("Exiting task");
            task.set_done();

            // restore current task
            self.current = Some(task);

            // switch back to core
            self.switch_core();

            unreachable!("Switched back to exited task");
        } else {
            panic!("Tried to exit, but there was no current task");
        }
    }

    fn switch_task(&mut self, mut task: Task) -> Task {
        if !self.in_core() {
            panic!("Tried to switch tasks while not in core task");
        }

        if task.inner.busy != 0 {
            panic!("Tried to execute a busy task");
        }

        unsafe {
            // save our
            asm!("fxsave $0"
                 : "=*m"(&mut _fxsave_task)
                 ::: "intel");

            ptr::copy(&mut _fxsave_task,
                      self.core.inner.context.fxsave.as_mut_ptr(),
                      self.core.inner.context.fxsave.len());

            ptr::copy(task.inner.context.fxsave.as_ptr(),
                      &mut _fxsave_task as *mut u8,
                      task.inner.context.fxsave.len());

            asm!("fxrstor $0"
                 :: "*m"(&_fxsave_task)
                 :: "intel");

            debug!("Executing task");

            task.inner.busy = !0;

            self.current = Some(task);

            _do_execute(&self.current.as_ref().unwrap().inner.context.regs,
                        &mut self.core.inner.busy,
                        &mut self.core.inner.context.regs);

            let mut task = self.current.take().unwrap();

            task.inner.busy = 0;

            debug!("Switched back");

            task
        }
    }

    fn switch_core(&mut self) {
        // NOTE: this mechanism will probably have to change

        // switch back to the core task
        if self.in_core() {
            panic!("Tried to switch back to core task while in core task");
        }

        let mut task = self.current
                           .as_mut()
                           .expect("Tried to switch to core, but there was no current task");

        if task.inner.busy == 0 {
            panic!("Tried to switch te core, but current task was not busy");
        }

        unsafe {
            // save our
            asm!("fxsave $0"
                 : "=*m"(&mut _fxsave_task)
                 ::: "intel");

            ptr::copy(&mut _fxsave_task as *mut u8,
                      task.inner.context.fxsave.as_mut_ptr(),
                      task.inner.context.fxsave.len());

            ptr::copy(self.core.inner.context.fxsave.as_ptr(),
                      &mut _fxsave_task as *mut u8,
                      self.core.inner.context.fxsave.len());

            asm!("fxrstor $0"
                 :: "*m"(&_fxsave_task)
                 :: "intel");

            debug!("Switching back to core");

            _do_execute(&self.core.inner.context.regs,
                        &mut task.inner.busy,
                        &mut task.inner.context.regs);

            debug!("Switched back to task");
        }
    }
}

impl Context {
    pub const fn empty() -> Context {
        Context {
            fxsave: [0; 0x200],
            regs: Regs::empty(),
        }
    }
}

impl TaskInner {
    pub fn create(level: PrivilegeLevel, entry: extern "C" fn() -> !, stack: Stack) -> TaskInner {
        // create a blank context
        let mut context = Context::empty();

        unsafe {
            // fxsave, use current floating point state in task
            // TODO: generate a compliant FPU state instead of just using the current one
            asm!("fxsave $0"
                 : "=*m"(&mut _fxsave_task)
                 ::: "intel");

            // copy fxsave area
            ptr::copy(&mut _fxsave_task,
                      context.fxsave.as_mut_ptr(),
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

        TaskInner {
            context: context,
            entry: entry,
            level: level,
            stack: stack,
            busy: 0,
            done: false,
        }
    }

    pub fn set_done(&mut self) {
        self.done = true;
    }

    pub fn is_done(&mut self) -> bool {
        self.done
    }
}
