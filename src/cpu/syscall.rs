use cpu::stack::Stack;

use constants::*;

// things that are clobbered by sysenter:
// RIP, RSP, CS, SS
// RDI, RSI, RDX, RCX

// branch, argument
// r8, r9

extern "C" {
    fn _sysenter_landing(rsp: u64, branch: u64, argument: u64) -> !;
    fn _sysenter_return(rsp: u64, result: u64) -> !;
    fn _sysenter_launch(branch: u64, argument: u64) -> u64;
    fn _sysenter_execute(rsp: u64, callback: extern "C" fn(u64) -> u64, argument: u64) -> !;
}

pub unsafe fn setup() -> Stack {
    // syscall handler doesn't need a huge stack
    let stack = Stack::create(0x10000);

    // write MSRs
    ::cpu::write_msr(SYSENTER_CS_MSR, CORE_CS as u64);
    ::cpu::write_msr(SYSENTER_EIP_MSR, _sysenter_landing as u64);
    ::cpu::write_msr(SYSENTER_ESP_MSR, stack.get_ptr() as u64);

    // return stack
    stack
}

pub extern "C" fn release() {
    trace!("release");
    unsafe {
        _sysenter_launch(0, 0);
    }
}

pub extern "C" fn exit() -> ! {
    trace!("exit");
    unsafe {
        _sysenter_launch(0, 1);
    }

    unreachable!("Exited task restarted");
}

pub extern "C" fn wait() {
    trace!("wait");
    unsafe {
        _sysenter_launch(0, 2);
    }
}

extern "C" fn release_callback(_: u64) -> u64 {
    // release the task
    ::cpu::task::release();

    1
}

extern "C" fn exit_callback(_: u64) -> u64 {
    // exit the task
    ::cpu::task::exit();
}

extern "C" fn wait_callback(_: u64) -> u64 {
    // wait
    ::cpu::task::wait();

    1
}

#[no_mangle]
pub unsafe extern "C" fn sysenter_handler(rsp: u64, branch: u64, argument: u64) -> ! {
    debug!("sysenter_handler reached, branch {:?} argument {:?}", branch, argument);

    match branch {
        0 => {
            // task interactions
            match argument {
                0 => {
                    // release
                    _sysenter_execute(rsp, release_callback, argument);
                },
                1 => {
                    // exit
                    _sysenter_execute(rsp, exit_callback, argument);
                },
                2 => {
                    // wait
                    _sysenter_execute(rsp, wait_callback, argument);
                },
                _ => {
                    error!("Unknown argument to branch 0: {}", argument);
                }
            }
        },
        _ => {
            error!("Unknown branch: {}", branch);
        }
    }

    _sysenter_return(rsp, 0);
}
