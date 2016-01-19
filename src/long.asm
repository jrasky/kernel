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
    global _interrupt
    global _interrupt_with_error

    extern kernel_main
    extern _boot_info
    extern interrupt

    section .text
    bits 64
_lstart:
    ;; Target of far jump to long mode

    ;; setup SSE
    call _setup_SSE

    ;; boot_info argument
    mov rdi, qword [_boot_info]

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

_interrupt:
    push 0x0                    ;push error code
_interrupt_with_error:  
    pop rdi                     ;error code
    pop rsi                     ;RIP
    pop rdx                     ;CS
    pop rcx                     ;RFLAGS
    pop r8                      ;RSP
    pop r9                      ;SS
    call interrupt
    ;; interrupt handler should not return
    mov al, "I"
    jmp _error
