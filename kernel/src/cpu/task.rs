use collections::{VecDeque, Vec};

#[cfg(not(test))]
use core::iter::{IntoIterator, Iterator};
#[cfg(not(test))]
use core::ptr;

#[cfg(test)]
use std::iter::{IntoIterator, Iterator};
#[cfg(test)]
use std::ptr;

use alloc::boxed::Box;
use alloc::arc::{Arc, Weak};

#[cfg(not(test))]
use core::cell::UnsafeCell;
#[cfg(test)]
use std::cell::UnsafeCell;

use spin::Mutex;

use cpu::stack::Stack;
use constants::*;

#[cfg(not(test))]
extern "C" {
    fn _do_execute(regs: *const Regs, core_regs: *mut Regs);
    fn _load_context(regs: *mut Regs);

    static mut _fxsave_task: [u8; FXSAVE_SIZE];
}

#[cfg(test)]
unsafe fn _do_execute(regs: *const Regs, core_regs: *mut Regs) {
    debug!("_do_execute(regs: {:?}, core_regs: {:?})", regs, core_regs);
}

unsafe extern "C" fn _dummy_entry() -> ! {
    unreachable!("Tried to entry dummy entry");
}

static mut MANAGER: Manager = Manager::new();

pub fn run_next() -> Result<TaskRef, RunNextResult> {
    unsafe { MANAGER.run_next() }
}

pub fn exit() -> ! {
    unsafe { MANAGER.exit() }
}

pub fn wait() {
    unsafe { MANAGER.wait() }
}

pub fn add(task: Task) -> TaskRef {
    unsafe { MANAGER.add(task) }
}

#[allow(dead_code)] // may be used in the future
pub fn switch_task(task: Task) -> Task {
    unsafe { MANAGER.switch_task(task) }
}

pub fn release() {
    unsafe { MANAGER.switch_core() }
}

pub enum RunNextResult {
    NoTasks,
    Blocked(TaskRef)
}

#[repr(u8)]
pub enum PrivilegeLevel {
    CORE = 0, // privileged instructions
    #[allow(dead_code)]
    DRIVER = 1, // identity page mapping, i/o
    #[allow(dead_code)]
    EXECUTIVE = 2, // program supervisor
    #[allow(dead_code)]
    USER = 3, // isolated
}

struct Manager {
    inner: *mut ManagerInner,
}

struct ManagerInner {
    core: TaskInner,
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
    inner: Arc<UnsafeCell<TaskInner>>
}

#[derive(Clone)]
pub struct TaskRef {
    inner: Weak<UnsafeCell<TaskInner>>
}

struct TaskInner {
    context: Context,
    #[allow(dead_code)]
    entry: unsafe extern "C" fn() -> !,
    #[allow(dead_code)]
    level: PrivilegeLevel,
    #[allow(dead_code)]
    stack: Stack,
    busy: u16,
    done: bool,
    blocked: bool
}

#[derive(Clone)]
pub struct Gate {
    inner: Arc<Mutex<GateInner>>
}

struct GateInner {
    tasks: Vec<TaskRef>
}

impl Drop for ManagerInner {
    fn drop(&mut self) {
        panic!("Tried to drop task manager");
    }
}

impl Gate {
    pub fn new<T: IntoIterator<Item=TaskRef>>(tasks: T) -> Gate {
        Gate {
            inner: Arc::new(Mutex::new(GateInner::new(tasks)))
        }
    }

    pub fn add_task(&mut self, task: TaskRef) {
        self.inner.lock().add_task(task)
    }

    pub fn finish(&mut self) {
        self.inner.lock().finish()
    }
}

impl GateInner {
    fn new<T: IntoIterator<Item=TaskRef>>(tasks: T) -> GateInner {
        GateInner {
            tasks: tasks.into_iter().collect()
        }
    }

    fn add_task(&mut self, task: TaskRef) {
        self.tasks.push(task);
    }

    fn finish(&mut self) {
        for mut task in self.tasks.drain(..) {
            task.unblock();
        }
    }
}

#[allow(dead_code)] // will use eventually
impl TaskRef {
    #[inline]
    pub fn dropped(&self) -> bool {
        self.get_inner().is_some()
    }

    #[inline]
    pub fn set_done(&mut self) {
        if let Some(inner) = self.get_mut() {
            inner.set_done()
        }
    }

    #[inline]
    pub fn is_done(&self) -> bool {
        if let Some(inner) = self.get_inner() {
            inner.is_done()
        } else {
            true
        }
    }

    #[inline]
    pub fn block(&mut self) {
        if let Some(inner) = self.get_mut() {
            inner.block()
        }
    }

    #[inline]
    pub fn unblock(&mut self) {
        if let Some(inner) = self.get_mut() {
            inner.unblock()
        }
    }

    #[inline]
    pub fn is_blocked(&self) -> bool {
        if let Some(inner) = self.get_inner() {
            inner.is_blocked()
        } else {
            false
        }
    }

    #[inline]
    fn get_mut(&mut self) -> Option<&mut TaskInner> {
        unsafe {
            self.inner.upgrade().map(|inner| inner.get().as_mut().unwrap())
        }
    }

    #[inline]
    fn get_inner(&self) -> Option<&TaskInner> {
        unsafe {
            self.inner.upgrade().map(|inner| inner.get().as_ref().unwrap())
        }
    }
}

impl Task {
    #[inline]
    pub fn create(level: PrivilegeLevel, entry: unsafe extern "C" fn() -> !, stack: Stack) -> Task {
        Task { 
            inner: Arc::new(UnsafeCell::new(TaskInner::create(level, entry, stack)))
        }
    }

    #[inline]
    pub fn get_ref(&self) -> TaskRef {
        TaskRef {
            inner: Arc::downgrade(&self.inner)
        }
    }

    #[inline]
    pub fn set_done(&mut self) {
        unsafe {self.inner.get().as_mut().unwrap().set_done()}
    }

    #[inline]
    pub fn is_done(&self) -> bool {
        unsafe {self.inner.get().as_ref().unwrap().is_done()}
    }

    #[inline]
    pub fn block(&mut self) {
        unsafe {self.inner.get().as_mut().unwrap().block()}
    }

    #[allow(dead_code)] // will use eventually
    #[inline]
    pub fn unblock(&mut self) {
        unsafe {self.inner.get().as_mut().unwrap().unblock()}
    }

    #[inline]
    pub fn is_blocked(&self) -> bool {
        unsafe {self.inner.get().as_mut().unwrap().is_blocked()}
    }

    #[inline]
    fn get_mut(&mut self) -> &mut TaskInner {
        unsafe {
            self.inner.get().as_mut().unwrap()
        }
    }

    #[inline]
    fn get_inner(&self) -> &TaskInner {
        unsafe {
            self.inner.get().as_ref().unwrap()
        }
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
    pub fn run_next(&mut self) -> Result<TaskRef, RunNextResult> {
        self.get_inner().run_next()
    }

    #[inline]
    pub fn add(&mut self, task: Task) -> TaskRef {
        self.get_inner().add(task)
    }

    #[inline]
    pub fn exit(&mut self) -> ! {
        self.get_inner().exit()
    }

    #[inline]
    pub fn wait(&mut self) {
        self.get_inner().wait()
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
            core: TaskInner {
                context: Context::empty(),
                entry: _dummy_entry,
                level: PrivilegeLevel::CORE,
                stack: unsafe { Stack::kernel() },
                busy: !0,
                done: false,
                blocked: false
            },
            tasks: VecDeque::new(),
            current: None
        }
    }

    #[inline]
    fn in_core(&self) -> bool {
        self.core.busy != 0
    }

    fn add(&mut self, task: Task) -> TaskRef {
        let rref = task.get_ref();
        self.tasks.push_back(task);
        rref
    }

    fn run_next(&mut self) -> Result<TaskRef, RunNextResult> {
        // get the next task
        if let Some(mut task) = self.tasks.pop_front() {
            // don't run if blocked
            if task.is_blocked() {
                let rref = task.get_ref();
                self.tasks.push_back(task);
                return Err(RunNextResult::Blocked(rref));
            }

            // switch to it
            task = self.switch_task(task);
            
            // get a ref to it
            let rref = task.get_ref();
            
            // add it if it isn't done
            if !task.is_done() {
                self.tasks.push_back(task);
            }
            
            // done
            Ok(rref)
        } else {
            // no tasks
            Err(RunNextResult::NoTasks)
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

    fn wait(&mut self) {
        if self.in_core() {
            panic!("Cannot block core task");
        }

        // set our status to blocked
        if let Some(task) = self.current.as_mut() {
            task.block();
        }

        // then switch out
        self.switch_core();
    }

    fn switch_task(&mut self, mut task: Task) -> Task {
        if !self.in_core() {
            panic!("Tried to switch tasks while not in core task");
        }


        if task.get_inner().busy != 0 {
            panic!("Tried to execute a busy task");
        }

        if task.get_inner().blocked {
            warn!("Tried to switch to a blocked task");
            return task;
        }

        unsafe {
            // save our
            #[cfg(not(test))]
            asm!("fxsave $0"
                 : "=*m"(_fxsave_task.as_mut_ptr())
                 ::: "intel");

            #[cfg(not(test))]
            ptr::copy(_fxsave_task.as_ptr(),
                      self.core.context.fxsave.as_mut_ptr(),
                      self.core.context.fxsave.len());

            #[cfg(not(test))]
            ptr::copy(task.get_inner().context.fxsave.as_ptr(),
                      _fxsave_task.as_mut_ptr(),
                      task.get_inner().context.fxsave.len());

            debug!("Executing task");

            task.get_mut().busy = !0;
            self.core.busy = 0;

            self.current = Some(task);

            #[cfg(not(test))]
            asm!("fxrstor $0"
                 :: "*m"(_fxsave_task.as_ptr())
                 :: "intel");

            _do_execute(&self.current.as_ref().unwrap().get_inner().context.regs,
                        &mut self.core.context.regs);

            let mut task = self.current.take().unwrap();

            #[cfg(not(test))]
            ptr::copy(self.core.context.fxsave.as_mut_ptr(),
                      _fxsave_task.as_mut_ptr(),
                      self.core.context.fxsave.len());

            #[cfg(not(test))]
            asm!("fxrstor $0"
                 :: "*m"(_fxsave_task.as_ptr())
                 :: "intel");

            debug!("Switched back");

            self.core.busy = !0;
            task.get_mut().busy = 0;

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

        if task.get_inner().busy == 0 {
            panic!("Tried to switch te core, but current task was not busy");
        }

        unsafe {
            // save our
            #[cfg(not(test))]
            asm!("fxsave $0"
                 : "=*m"(_fxsave_task.as_mut_ptr())
                 ::: "intel");

            #[cfg(not(test))]
            ptr::copy(_fxsave_task.as_ptr(),
                      task.get_mut().context.fxsave.as_mut_ptr(),
                      task.get_mut().context.fxsave.len());

            #[cfg(not(test))]
            ptr::copy(self.core.context.fxsave.as_ptr(),
                      _fxsave_task.as_mut_ptr(),
                      self.core.context.fxsave.len());

            debug!("Switching back to core");

            #[cfg(not(test))]
            asm!("fxrstor $0"
                 :: "*m"(_fxsave_task.as_ptr())
                 :: "intel");

            _do_execute(&self.core.context.regs,
                        &mut task.get_mut().context.regs);

            #[cfg(not(test))]
            ptr::copy(task.get_inner().context.fxsave.as_ptr(),
                      _fxsave_task.as_mut_ptr(),
                      task.get_inner().context.fxsave.len());

            #[cfg(not(test))]
            asm!("fxrstor $0"
                 :: "*m"(_fxsave_task.as_ptr())
                 :: "intel");

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
    pub fn create(level: PrivilegeLevel, entry: unsafe extern "C" fn() -> !, stack: Stack) -> TaskInner {
        // create a blank context
        let mut context = Context::empty();

        #[cfg(not(test))]
        unsafe {
            // fxsave, use current floating point state in task
            // TODO: generate a compliant FPU state instead of just using the current one
            asm!("fxsave $0"
                 : "=*m"(_fxsave_task.as_mut_ptr())
                 ::: "intel");

            // copy fxsave area
            ptr::copy(_fxsave_task.as_ptr(),
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
            blocked: false
        }
    }

    #[inline]
    pub fn set_done(&mut self) {
        self.done = true;
    }

    #[inline]
    pub fn is_done(&self) -> bool {
        self.done
    }

    #[inline]
    pub fn block(&mut self) {
        self.blocked = true;
    }

    #[inline]
    pub fn unblock(&mut self) {
        self.blocked = false;
    }

    #[inline]
    pub fn is_blocked(&self) -> bool {
        self.blocked
    }
}
