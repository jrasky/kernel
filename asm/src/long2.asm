    global do_kernel_main

    extern _boot_stack

    section .text exec
    bits 64
do_kernel_main:
    ;; reset the kernel stack to the beginning
    mov rsp, _boot_stack
    ;; align the stack, just in case
    and rsp, -16

    ;; kernel_main's address is split into eax (low bits) and ecx (high bits)
    ;; reconstruct it
    shl rcx, 32
    or rax, rcx

    ;; call kernel_main, whose address is in rax
    call rax

    ;; hang if the kernel returns
_hang:
    cli
    hlt
    jmp _hang
