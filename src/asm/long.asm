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

    extern kernel_main
    extern _boot_info
    extern _fxsave_trap
    extern _fxsave_int
    extern _fxsave_task
    extern interrupt_breakpoint
    extern interrupt_general_protection_fault

    section .text
    bits 64
_lstart:
    ;; Target of far jump to long mode

    ;; setup SSE
    call _setup_SSE

    ;; boot_info argument
    mov rdi, qword [_boot_info]

    ;; align stack
    and rsp, -16

    ;; start kernel
    call kernel_main

    ;; kernel_main returned, error "X"
    mov al, "X"
    jmp _error

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

    ;; check for, then enable SSE
    ;; Not supported: error "a"
_setup_SSE:
    ;; check for SSE
    mov rax, 0x1
    cpuid
    test edx, 1<<25
    jz .no_SSE

    ;; enable SSE
    mov rax, cr0
    and ax, 0xFFFB              ;clear coprocessor emulation CR0.EM
    or ax, 0x2                  ;set coprocessor monitoring CR0.MP
    mov cr0, rax
    mov rax, cr4
    or ax, 3 << 9               ;set CR4.OSFXSR and CR4.OSXMMEXCPT at the same time
    mov cr4, rax

    ret
.no_SSE:
    mov al, "a"
    jmp _error

_reload_segments:
    push 0x08                   ;second selector is code selector
    push .target
    o64 retf
.target:
    mov ax, 0x10                ;third selector is data selector
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    ret

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
%endmacro

;;; Some interrupts

_bp_handler:
    interrupt_handler interrupt_breakpoint
    
_gp_handler:
    jmp .with_error             ;has an error code
    interrupt_handler interrupt_general_protection_fault
