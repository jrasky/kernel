extern "C" {
    pub fn test_task_entry() -> !;

    pub static _image_begin: u8;
    pub static _image_end: u8;
    pub static _gen_max_paddr: u64;
    pub static _gen_segments_size: u64;
    pub static _gen_page_tables: u64;
    pub static _gen_segments: u8;

    pub static _kernel_top: u8;
    pub static _kernel_end: u8;
    pub static _bss_top: u8;
    pub static _long_stack: u8;
    pub static _rodata_top: u8;
    pub static _rodata_end: u8;
    pub static _data_top: u8;
    pub static _data_end: u8;
    
    pub fn _swap_pages(cr3: u64);
    pub fn _init_pages();

    pub fn _bp_handler();
    pub fn _gp_handler();
    pub fn _pf_handler();

    pub fn _bp_early_handler();
    pub fn _gp_early_handler();
    pub fn _pf_early_handler();
}

#[cfg(not(test))]
extern "C" {
    pub fn _sysenter_landing(rsp: u64, branch: u64, argument: u64) -> !;
    pub fn _syscall_landing(rsp: u64, branch: u64, argument: u64) -> !;
    pub fn _sysenter_return(rsp: u64, result: u64) -> !;
    pub fn _syscall_launch(branch: u64, argument: u64) -> u64;
    pub fn _sysenter_execute(rsp: u64, callback: extern "C" fn(u64) -> u64, argument: u64) -> !;
}

#[cfg(test)]
unsafe fn _sysenter_landing(_: u64, _: u64, _: u64) -> ! {
    unreachable!("sysenter landing called");
}

#[cfg(test)]
unsafe fn _syscall_landing(_: u64, _: u64, _: u64) -> ! {
    unreachable!("syscall landing called");
}

#[cfg(test)]
unsafe fn _sysenter_return(_: u64, result: u64) -> ! {
    panic!(result);
}

#[cfg(test)]
unsafe fn _sysenter_execute(_: u64, callback: extern "C" fn(u64) -> u64, argument: u64) -> ! {
    panic!(callback(argument));
}
