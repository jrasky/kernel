;;; long.asm
;;;
;;; Copyright (C) 2015 Jerome Rasky
;;; 
;;; Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
;;; http://www.apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT
;;; or http://opensource.org/licenses/MIT>, at your option. This file may not be
;;; copied, modified, or distributed except according to those terms.

    global _lstart
    global _reload_segments
    global _bp_handler
    global _gp_handler
    global _pf_handler
    global _do_execute
    global _load_context
    global _sysenter_landing
    global _sysenter_return
    global _sysenter_launch
    global _sysenter_execute
    global _swap_pages
    global _long_stack
    global _fxsave_trap
    global _fxsave_task
    global _syscall_launch
    global _syscall_landing
    global _bp_early_handler
    global _gp_early_handler
    global _pf_early_handler
    global _reserve_slab
	global _entry_stack_end
	global _entry_stack

    extern kernel_main
    extern _boot_info
    extern _boot_info_size
    extern interrupt_breakpoint
    extern interrupt_general_protection_fault
    extern interrupt_page_fault
    extern early_interrupt_breakpoint
    extern early_interrupt_general_protection_fault
    extern early_interrupt_page_fault
    extern sysenter_handler
    extern SYSCALL_STACK

    section .bss nobits
    align 16
_fxsave_int:    resb 0x200
    align 16
_fxsave_trap:   resb 0x200
    align 16
_fxsave_task:   resb 0x200
    align 8
_reserve_slab:  resb 0x8000     ;must match constants.rs

	;; early stack used for entry
_entry_stack_end:
    resb 0xf000
_entry_stack:


    section .text exec
    bits 64

;;; Internal utility functions

_hang:
    cli
    hlt
    jmp _hang

    ;; Prints 'ERROR: ' and the given error code and hangs
_error:
    mov rbx, 0x4f4f4f524f524f45
    mov [0xb8000], rbx
    mov rbx, 0x4f204f204f3a4f52
    mov [0xb8008], rbx
    mov byte [0xb800e], al
    jmp _hang
    
;;; Interrupt handler macro

%macro interrupt_handler 1
    push 0x0                    ;push null error to ensure consistent stack frame
.with_error:
    ;; push general-purpose registers
	push r15
	push r14
    push r13
    push r12
    push r11
    push r10
    push r9
    push r8
    push rdi
    push rsi
    push rbp
    push rdx
    push rcx
    push rbx
    push rax

    ;; fxsave
    fxsave [_fxsave_int]

    ;; first argument is the position of the stack, which contains all the context
    ;; needed to unwind
    mov rdi, rsp
    ;; copy stack pointer to rbp, so it's saved after the interrupt handler
    mov rbp, rsp

    ;; align stack
    and rsp, -16

    ;; interrupt handler
    call %1

    ;; de-align stack
    mov rsp, rbp

    ;; fxrstor
    fxrstor [_fxsave_int]

    ;; restore old registers
    pop rax
    pop rbx
    pop rcx
    pop rdx
    pop rbp
    pop rsi
    pop rdi
    pop r8
    pop r9
    pop r10
    pop r11
    pop r12
    pop r13
    pop r14
    pop r15

    ;; skip error code
    add rsp, 0x08
    
    ;; iret
    iretq

    mov al, "I"
    jmp _error
%endmacro

;;; Some interrupts

_bp_handler:
    interrupt_handler interrupt_breakpoint
    
_gp_handler:
    jmp .with_error             ;has an error code
    interrupt_handler interrupt_general_protection_fault

_pf_handler:
    jmp .with_error             ;has an error code
    interrupt_handler early_interrupt_page_fault

_bp_early_handler:
    interrupt_handler early_interrupt_breakpoint
    
_gp_early_handler:
    jmp .with_error             ;has an error code
    interrupt_handler early_interrupt_general_protection_fault

_pf_early_handler:
    jmp .with_error             ;has an error code
    interrupt_handler early_interrupt_page_fault
