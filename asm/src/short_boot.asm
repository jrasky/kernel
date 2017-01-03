    extern bootstrap

    global _start
    global _boot_stack
    global _reserve_slab

	section .bss nobits
    align 16
_boot_stack_end:
    resb 0x20000
_boot_stack:
    align 8
_reserve_slab:  resb 0x8000     ;must match constants.rs

    section .text exec

_start:
    ;; grub entry point

    ;; set up stack
    mov esp, _boot_stack
    ;; align stack for arguments
    sub esp, 8
    and esp, -16
    ;; make space for argumens
    add esp, 8

    ;; push arguments
    push ebx
    push eax
    
    ;; call bootstrap
    call bootstrap

    ;; hang if bootstrap returns

_hang:
    cli
    hlt
    jmp _hang
