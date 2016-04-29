    extern bootstrap

    global _start

	section .bss nobits
_boot_stack_end:
    resb 0xf000
_boot_stack:

    section .text exec

_start:
    ;; grub entry point

    ;; set up stack
    mov esp, _boot_stack

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
