#![no_std]

#[macro_use]
extern crate log;

#[link(name = "user-asm", kind = "static")]
extern "C" {
    fn _syscall_launch(branch: u64, argument: u64) -> u64;
}

pub fn release() {
    trace!("release");
    unsafe {
        _syscall_launch(0, 0);
    }
}

pub fn exit() -> ! {
    trace!("exit");
    unsafe {
        _syscall_launch(0, 1);
    }

    unreachable!("Returned to exited task");
}

pub fn wait() {
    trace!("wait");
    unsafe {
        _syscall_launch(0, 2);
    }
}

pub fn log() {
    unimplemented!();
}
