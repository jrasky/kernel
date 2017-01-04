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


;;; load context object
_load_context:
    ;; rdi is a pointer to the regs field of the context object
    mov [rdi + 0x00], rax
    mov [rdi + 0x08], rbx
    mov [rdi + 0x10], rcx
    mov [rdi + 0x18], rdx
    mov [rdi + 0x20], rbp
    mov [rdi + 0x28], rsi
    mov [rdi + 0x30], rdi
    mov [rdi + 0x38], r8
    mov [rdi + 0x40], r9
    mov [rdi + 0x48], r10
    mov [rdi + 0x50], r11
    mov [rdi + 0x58], r12
    mov [rdi + 0x60], r13
    mov [rdi + 0x68], r14
    mov [rdi + 0x70], r15
    pushfq
    pop QWORD [rdi + 0x78]            ;rflags
    mov QWORD [rdi + 0x80], .end      ;rip
    mov [rdi + 0x88], rsp
    mov [rdi + 0x90], cs
    mov [rdi + 0x92], ss
    mov [rdi + 0x94], ds
    mov [rdi + 0x96], es
    mov [rdi + 0x98], fs
    mov [rdi + 0x9a], gs
.end:
    ret

;;; Context switch and far jump

_do_execute_landing:
    xchg rdi, rsi
    ret

_do_execute:
    ;; rdi is a pointer to the context after the fxsave area
    ;; rsi is a pointer to the return context registers

    ;; note that this makes bad assumptions
    ;; fix with swapgs etc once I get around to it

    ;; save return context
    xchg rdi, rsi
    call _load_context
    xchg rdi, rsi

    ;; save stack here
    mov [rsi + 0x88], rsp
    ;; set return point
    mov QWORD [rsi + 0x80], _do_execute_landing

    ;; rax: rdi + 0x00
    mov rbx, [rdi + 0x08]
    mov rcx, [rdi + 0x10]
    mov rdx, [rdi + 0x18]
    mov rbp, [rdi + 0x20]
    mov rsi, [rdi + 0x28]
    ;; mov rdi, [rdi + 0x30]
    mov r8, [rdi + 0x38]
    mov r9, [rdi + 0x40]
    mov r10, [rdi + 0x48]
    mov r11, [rdi + 0x50]
    mov r12, [rdi + 0x58]
    mov r13, [rdi + 0x60]
    mov r14, [rdi + 0x68]
    mov r15, [rdi + 0x70]
    ;; rflags: 0x78
    ;; rip: 0x80
    ;; rsp: 0x88
    ;; cs: 0x90
    ;; ss: 0x92
    mov ds, [rdi + 0x94]
    mov es, [rdi + 0x96]
    mov fs, [rdi + 0x98]
    mov gs, [rdi + 0x9a]

    ;; set up stack for iret
    xor rax, rax
    mov ax, WORD [rdi + 0x92]
    push rax                    ;ss
    push QWORD [rdi + 0x88]     ;rsp
    push QWORD [rdi + 0x78]     ;rflags
    xor rax, rax
    mov ax, WORD [rdi + 0x90]   ;cs
    push rax
    push QWORD [rdi + 0x80]     ;rip

    ;; restore our scratch registers
    mov rax, [rdi]
    mov rdi, [rdi + 0x30]

    ;; iret to procedure
    iretq

    mov al, "E"
    jmp _error

_syscall_landing:
    ;; switch stack
    mov rsp, QWORD [SYSCALL_STACK]

_sysenter_landing:
    ;; align stack
    and rsp, -16

    ;; call handler procedure
    call sysenter_handler

    ;; should not return
    mov al, "S"
    jmp _error

_sysenter_return:
    mov rax, rsi                ;result
    mov rsp, rdi
    iretq

    mov al, "R"
    jmp _error

_sysenter_execute:
    ;; execute a function on another stack
    ;; then return to original procedure

    ;; rdi: new rsp
    ;; rsi: callback
    ;; rdx: argument

    mov rsp, rdi
    mov rdi, rdx

    ;; align stack, just in case
    mov rbp, rsp
    and rsp, -16

    call rsi

    ;; restore old rsp
    mov rsp, rbp
    ;; rax should have the result of the function
    iretq

    mov al, "E"
    jmp _error

_swap_pages:
    mov cr3, rdi
    ret