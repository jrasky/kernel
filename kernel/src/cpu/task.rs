use include::*;

use paging::{Region, Allocator, Layout, Segment, Table, Base};

use cpu::stack::Stack;

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

pub fn switch_task(task: Task) -> Task {
    unsafe { MANAGER.switch_task(task) }
}

pub fn current() -> TaskRef {
    unsafe { MANAGER.current() }
}

pub fn switch_core() {
    unsafe { MANAGER.switch_core() }
}

pub fn to_physical(addr: usize) -> Option<usize> {
    unsafe { MANAGER.to_physical(addr) }
}

pub fn allocate(size: usize, align: usize) -> Option<Region> {
    unsafe { MANAGER.allocate(size, align) }
}

pub fn release(region: Region) -> bool {
    unsafe { MANAGER.release(region) }
}

pub fn register(region: Region) -> bool {
    unsafe { MANAGER.register(region) }
}

pub fn forget(region: Region) -> bool {
    unsafe { MANAGER.forget(region) }
}

pub fn set_used(region: Region) -> bool {
    unsafe { MANAGER.set_used(region) }
}

pub enum RunNextResult {
    NoTasks,
    Blocked(TaskRef)
}

#[repr(u8)]
pub enum PrivilegeLevel {
    CORE = 0, // privileged instructions
    DRIVER = 1, // identity page mapping, i/o
    EXECUTIVE = 2, // program supervisor
    USER = 3, // isolated
}

struct Manager {
    inner: *mut ManagerInner,
}

struct ManagerInner {
    core: Task,
    // physical memory
    memory: Allocator,
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
    entry: unsafe extern "C" fn() -> !,
    level: PrivilegeLevel,
    stack: Stack,
    memory: Arc<RefCell<Layout>>,
    tables: Arc<RefCell<DirectBuilder>>,
    parent: Option<TaskRef>,
    busy: u16,
    done: bool,
    blocked: bool
}

struct DirectBuilder {
    tables: Vec<Shared<Table>>,
    to_virtual: BTreeMap<usize, usize>
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

impl Base for DirectBuilder {
    fn to_physical(&self, address: usize) -> Option<usize> {
        to_physical(address)
    }

    fn to_virtual(&self, address: usize) -> Option<usize> {
        self.to_virtual.get(&address).map(|addr| *addr)
    }

    unsafe fn new_table(&mut self) -> Shared<Table> {
        let table: Shared<Table> = Shared::new(heap::allocate(mem::size_of::<Table>(), 0x1000) as *mut Table);
        self.tables.push(table);
        let physical = self.to_physical(*table as usize).expect("No physical mapping for table");
        self.to_virtual.insert(physical, *table as usize);
        table
    }

    fn clear(&mut self) {
        self.to_virtual.clear();

        for table in self.tables.drain(..) {
            unsafe {heap::deallocate(*table as *mut _, mem::size_of::<Table>(), 0x1000)};
        }
    }
}

impl DirectBuilder {
    fn new() -> DirectBuilder {
        DirectBuilder {
            tables: vec![],
            to_virtual: BTreeMap::new()
        }
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

impl TaskRef {
    #[inline]
    pub fn dropped(&self) -> bool {
        self.get_inner().is_some()
    }

    #[inline]
    pub fn to_physical(&self, addr: usize) -> Option<usize> {
        if let Some(inner) = self.get_inner() {
            inner.to_physical(addr)
        } else {
            None
        }
    }

    #[inline]
    pub fn allocate(&mut self, size: usize, align: usize) -> Option<Region> {
        if let Some(inner) = self.get_mut() {
            inner.allocate(size, align)
        } else {
            None
        }
    }

    #[inline]
    pub fn map(&mut self, segment: Segment) -> bool {
        if let Some(inner) = self.get_mut() {
            inner.map(segment)
        } else {
            false
        }
    }

    #[inline]
    pub fn unmap(&mut self, segment: Segment) -> bool {
        if let Some(inner) = self.get_mut() {
            inner.unmap(segment)
        } else {
            false
        }
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
    fn memory(&self) -> Option<Arc<RefCell<Layout>>> {
        if let Some(inner) = self.get_inner() {
            Some(inner.memory())
        } else {
            None
        }
    }

    #[inline]
    fn tables(&self) -> Option<Arc<RefCell<DirectBuilder>>> {
        if let Some(inner) = self.get_inner() {
            Some(inner.tables())
        } else {
            None
        }
    }

    #[inline]
    fn root_ref(&self) -> TaskRef {
        if let Some(inner) = self.get_inner() {
            if let Some(parent) = inner.get_parent() {
                return parent;
            }
        }

        // else
        self.clone()
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
    pub fn process(level: PrivilegeLevel, entry: unsafe extern "C" fn() -> !,
                   stack: Stack, memory: Region) -> Task {
        let mut layout = Layout::new();
        layout.register(memory);

        Task::create(level, entry, stack, Arc::new(RefCell::new(layout)),
                     Arc::new(RefCell::new(DirectBuilder::new())), None)
    }

    #[inline]
    pub fn thread(level: PrivilegeLevel, entry: unsafe extern "C" fn() -> !,
                  stack: Stack, parent: TaskRef) -> Task {
        if let Some(layout) = parent.memory() {
            if let Some(tables) = parent.tables() {
                return Task::create(level, entry, stack, layout, tables, Some(parent));
            }
        }

        panic!("Tried to create thread on dead task");
    }

    #[inline]
    fn create(level: PrivilegeLevel, entry: unsafe extern "C" fn() -> !, stack: Stack,
              memory: Arc<RefCell<Layout>>, tables: Arc<RefCell<DirectBuilder>>,
              parent: Option<TaskRef>) -> Task {
        Task {
            inner: Arc::new(UnsafeCell::new(TaskInner::create(level, entry, stack, memory, tables, parent)))
        }
    }

    fn root_ref(&self) -> TaskRef {
        if let Some(parent) = unsafe {self.inner.get().as_ref().unwrap().get_parent()} {
            parent
        } else {
            self.get_ref()
        }
    }

    #[inline]
    pub fn get_ref(&self) -> TaskRef {
        TaskRef {
            inner: Arc::downgrade(&self.inner)
        }
    }

    #[inline]
    fn memory(&self) -> Arc<RefCell<Layout>> {
        unsafe {self.inner.get().as_ref().unwrap().memory()}
    }

    #[inline]
    fn tables(&self) -> Arc<RefCell<DirectBuilder>> {
        unsafe {self.inner.get().as_ref().unwrap().tables()}
    }

    #[inline]
    fn set_busy(&mut self, busy: bool) {
        unsafe {self.inner.get().as_mut().unwrap().set_busy(busy)}
    }

    #[inline]
    fn is_busy(&self) -> bool {
        unsafe {self.inner.get().as_ref().unwrap().is_busy()}
    }

    #[inline]
    fn fxsave_ptr(&self) -> *const u8 {
        unsafe {self.inner.get().as_ref().unwrap().fxsave_ptr()}
    }

    #[inline]
    fn fxsave_mut_ptr(&mut self) -> *mut u8 {
        unsafe {self.inner.get().as_mut().unwrap().fxsave_mut_ptr()}
    }

    #[inline]
    fn regs_ptr(&self) -> *const Regs {
        unsafe {self.inner.get().as_ref().unwrap().regs_ptr()}
    }

    #[inline]
    fn regs_mut_ptr(&mut self) -> *mut Regs {
        unsafe {self.inner.get().as_mut().unwrap().regs_mut_ptr()}
    }

    #[inline]
    pub fn to_physical(&self, addr: usize) -> Option<usize> {
        unsafe {self.inner.get().as_ref().unwrap().to_physical(addr)}
    }

    #[inline]
    pub fn allocate(&mut self, size: usize, align: usize) -> Option<Region> {
        unsafe {self.inner.get().as_mut().unwrap().allocate(size, align)}
    }

    #[inline]
    pub fn map(&mut self, segment: Segment) -> bool {
        unsafe {self.inner.get().as_mut().unwrap().map(segment)}
    }

    #[inline]
    pub fn unmap(&mut self, segment: Segment) -> bool {
        unsafe {self.inner.get().as_mut().unwrap().unmap(segment)}
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

    fn as_ptr(&self) -> *const Regs {
        self as *const _
    }

    fn as_mut_ptr(&mut self) -> *mut Regs {
        self as *mut _
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
    pub fn to_physical(&mut self, addr: usize) -> Option<usize> {
        self.get_inner().to_physical(addr)
    }

    #[inline]
    pub fn set_used(&mut self, region: Region) -> bool {
        self.get_inner().set_used(region)
    }

    #[inline]
    pub fn allocate(&mut self, size: usize, align: usize) -> Option<Region> {
        self.get_inner().allocate(size, align)
    }

    #[inline]
    pub fn release(&mut self, region: Region) -> bool {
        self.get_inner().release(region)
    }

    #[inline]
    pub fn register(&mut self, region: Region) -> bool {
        self.get_inner().register(region)
    }

    #[inline]
    pub fn forget(&mut self, region: Region) -> bool {
        self.get_inner().forget(region)
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

    #[inline]
    pub fn current(&mut self) -> TaskRef {
        self.get_inner().current()
    }
}

impl ManagerInner {
    fn new() -> ManagerInner {
        // don't map things before the heap
        let mut core = Task::process(PrivilegeLevel::CORE, _dummy_entry, unsafe { Stack::kernel() },
                                     Region::new(HEAP_BEGIN, CORE_SIZE - (HEAP_BEGIN - CORE_BEGIN)));

        // core is initially busy
        core.set_busy(true);

        ManagerInner {
            core: core,
            memory: Allocator::new(),
            tasks: VecDeque::new(),
            current: None
        }
    }

    #[inline]
    fn in_core(&self) -> bool {
        self.core.is_busy()
    }

    #[inline]
    fn set_used(&mut self, region: Region) -> bool {
        self.memory.set_used(region)
    }

    #[inline]
    fn allocate(&mut self, size: usize, align: usize) -> Option<Region> {
        self.memory.allocate(size, align)
    }

    #[inline]
    fn release(&mut self, region: Region) -> bool {
        self.memory.release(region)
    }

    #[inline]
    fn register(&mut self, region: Region) -> bool {
        self.memory.register(region)
    }

    #[inline]
    fn forget(&mut self, region: Region) -> bool {
        self.memory.forget(region)
    }

    fn to_physical(&self, addr: usize) -> Option<usize> {
        if self.in_core() {
            self.core.to_physical(addr)
        } else if let Some(ref task) = self.current {
            task.to_physical(addr)
        } else {
            unreachable!("Core task was not busy, but there was to current task");
        }
    }

    fn current(&self) -> TaskRef {
        if self.in_core() {
            self.core.get_ref()
        } else if let Some(ref task) = self.current {
            task.get_ref()
        } else {
            unreachable!("No busy task, but also not in core");
        }
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

        if task.is_busy() {
            panic!("Tried to execute a busy task");
        }

        if task.is_blocked() {
            warn!("Tried to switch to a blocked task");
            return task;
        }

        unsafe {
            // save our register state
            #[cfg(not(test))]
            asm!("fxsave $0"
                 : "=*m"(_fxsave_task.as_mut_ptr())
                 ::: "intel");

            // load the task's register state
            #[cfg(not(test))]
            ptr::copy(_fxsave_task.as_ptr(),
                      self.core.fxsave_mut_ptr(),
                      0x200);

            #[cfg(not(test))]
            ptr::copy(task.fxsave_ptr(),
                      _fxsave_task.as_mut_ptr(),
                      0x200);

            debug!("Executing task");

            task.set_busy(true);
            self.core.set_busy(false);

            self.current = Some(task);

            #[cfg(not(test))]
            asm!("fxrstor $0"
                 :: "*m"(_fxsave_task.as_ptr())
                 :: "intel");

            _do_execute(self.current.as_ref().unwrap().regs_ptr(),
                        self.core.regs_mut_ptr());

            let mut task = self.current.take().unwrap();

            #[cfg(not(test))]
            ptr::copy(self.core.fxsave_mut_ptr(),
                      _fxsave_task.as_mut_ptr(),
                      0x200);

            #[cfg(not(test))]
            asm!("fxrstor $0"
                 :: "*m"(_fxsave_task.as_ptr())
                 :: "intel");

            debug!("Switched back");

            self.core.set_busy(true);
            task.set_busy(false);

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

        if !task.is_busy() {
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
                      task.fxsave_mut_ptr(),
                      0x200);

            #[cfg(not(test))]
            ptr::copy(self.core.fxsave_ptr(),
                      _fxsave_task.as_mut_ptr(),
                      0x200);

            debug!("Switching back to core");

            #[cfg(not(test))]
            asm!("fxrstor $0"
                 :: "*m"(_fxsave_task.as_ptr())
                 :: "intel");

            _do_execute(self.core.regs_ptr(),
                        task.regs_mut_ptr());

            #[cfg(not(test))]
            ptr::copy(task.fxsave_ptr(),
                      _fxsave_task.as_mut_ptr(),
                      0x200);

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

    #[inline]
    fn fxsave_ptr(&self) -> *const u8 {
        self.fxsave.as_ptr()
    }

    #[inline]
    fn fxsave_mut_ptr(&mut self) -> *mut u8 {
        self.fxsave.as_mut_ptr()
    }

    #[inline]
    fn regs_ptr(&self) -> *const Regs {
        self.regs.as_ptr()
    }

    #[inline]
    fn regs_mut_ptr(&mut self) -> *mut Regs {
        self.regs.as_mut_ptr()
    }
}

impl TaskInner {
    fn create(level: PrivilegeLevel, entry: unsafe extern "C" fn() -> !, stack: Stack,
              memory: Arc<RefCell<Layout>>, tables: Arc<RefCell<DirectBuilder>>,
              parent: Option<TaskRef>) -> TaskInner {
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
            memory: memory,
            tables: tables,
            parent: parent,
            busy: 0,
            done: false,
            blocked: false
        }
    }

    #[inline]
    fn memory(&self) -> Arc<RefCell<Layout>> {
        self.memory.clone()
    }

    #[inline]
    fn tables(&self) -> Arc<RefCell<DirectBuilder>> {
        self.tables.clone()
    }

    #[inline]
    fn get_parent(&self) -> Option<TaskRef> {
        if let Some(ref parent) = self.parent {
            Some(parent.clone())
        } else {
            None
        }
    }

    #[inline]
    fn to_physical(&self, addr: usize) -> Option<usize> {
        self.memory.borrow().to_physical(addr)
    }

    #[inline]
    fn fxsave_ptr(&self) -> *const u8 {
        self.context.fxsave_ptr()
    }

    #[inline]
    fn fxsave_mut_ptr(&mut self) -> *mut u8 {
        self.context.fxsave_mut_ptr()
    }

    #[inline]
    fn regs_ptr(&self) -> *const Regs {
        self.context.regs_ptr()
    }

    #[inline]
    fn regs_mut_ptr(&mut self) -> *mut Regs {
        self.context.regs_mut_ptr()
    }

    #[inline]
    fn allocate(&mut self, size: usize, align: usize) -> Option<Region> {
        self.memory.borrow_mut().allocate(size, align)
    }

    #[inline]
    fn map(&mut self, segment: Segment) -> bool {
        self.memory.borrow_mut().insert(segment)
    }

    #[inline]
    fn unmap(&mut self, segment: Segment) -> bool {
        self.memory.borrow_mut().remove(segment)
    }

    #[inline]
    fn set_busy(&mut self, busy: bool) {
        if busy {
            self.busy = !0;
        } else {
            self.busy = 0;
        }
    }

    #[inline]
    fn is_busy(&self) -> bool {
        self.busy != 0
    }

    #[inline]
    fn set_done(&mut self) {
        self.done = true;
    }

    #[inline]
    fn is_done(&self) -> bool {
        self.done
    }

    #[inline]
    fn block(&mut self) {
        self.blocked = true;
    }

    #[inline]
    fn unblock(&mut self) {
        self.blocked = false;
    }

    #[inline]
    fn is_blocked(&self) -> bool {
        self.blocked
    }
}
