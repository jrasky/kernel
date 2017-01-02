    global _start
    global _early_stack_end
    global _early_stack

    extern kernel_main

    section .bss nobits
_early_stack_end:
    resb 0xf000
_early_stack:

    section .text exec
    bits 64
_start:
    ;; Target of far jump to long mode
    ;; rdi should theoretically contain the boot proto pointer

    ;; set up the kernel stack
    mov rsp, _early_stack
    ;; align the stack, just in case
    and rsp, -16

    ;; call kernel_main
    call kernel_main

    ;; hang if the kernel returns
_hang:
    cli
    hlt
    jmp _hang
