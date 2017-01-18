use constants::*;

use log;

#[cfg(test)]
use std::panic;

use c;

use cpu;

use kernel_std::cpu::stack::Stack;

// things that are clobbered by sysenter:
// RIP, RSP, CS, SS
// RDI, RSI, RDX, RCX

// branch, argument
// r8, r9

#[no_mangle]
pub static mut SYSCALL_STACK: u64 = 0;

pub unsafe fn setup() -> Stack {
    // syscall handler doesn't need a huge stack
    let stack = Stack::create(0xf000);

    // write MSRs
    util::write_msr(SYSENTER_CS_MSR, CORE_CS as u64);
    util::write_msr(SYSENTER_EIP_MSR, c::_sysenter_landing as u64);
    util::write_msr(SYSENTER_ESP_MSR, stack.get_ptr() as u64);

    util::write_msr(STAR_MSR, (CORE_CS as u64) << 32);
    util::write_msr(LSTAR_MSR, c::_syscall_landing as u64);
    SYSCALL_STACK = stack.get_ptr() as u64;

    // return stack
    stack
}

extern "C" fn release_callback(_: u64) -> u64 {
    // release the task
    cpu::task::switch_core();

    1
}

extern "C" fn exit_callback(_: u64) -> u64 {
    // exit the task
    cpu::task::exit();
}

extern "C" fn wait_callback(_: u64) -> u64 {
    // wait
    cpu::task::wait();

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
                    c::_sysenter_execute(rsp, release_callback, argument);
                },
                1 => {
                    // exit
                    c::_sysenter_execute(rsp, exit_callback, argument);
                },
                2 => {
                    // wait
                    c::_sysenter_execute(rsp, wait_callback, argument);
                },
                _ => {
                    error!("Unknown argument to branch 0: {}", argument);
                    c::_sysenter_return(rsp, 0);
                }
            }
        },
        /*
        1 => {
            // log
            // argument is pointer to log::Request structure
            let request: &log::Request = (argument as *const log::Request).as_ref().unwrap();
            log::log(request.level, &request.location,
                     &request.target, &request.message);
            // done
            c::_sysenter_return(rsp, 1)
        },
         */
        _ => {
            error!("Unknown branch: {}", branch);
            c::_sysenter_return(rsp, 0);
        }
    }
}
