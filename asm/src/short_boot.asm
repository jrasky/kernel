	COM1 equ 0x3f8

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

_test_multiboot:
    cmp eax, 0x36D76289
    jne .no_multiboot
    ret
.no_multiboot:
    mov al, "0"
    jmp _error

_test_cpuid:
    pushfd                      ; store flags register
    pop eax                     ; restore A-register
    mov ecx, eax                ; C = A
    xor eax, 1 << 21            ; Flip bit 21
    push eax                    ; store A
    popfd                       ; restore flags
    pushfd                      ; store flags
    pop eax                     ; restore A
    push ecx                    ; store C
    popfd                       ; restore flags
    xor eax, ecx                ; xor A and C
    jz .no_cpuid                ; if zero flag is set, no CPUID
    ret
.no_cpuid:
    mov al, "1"
    jmp _error

_test_long_mode:
    mov eax, 0x80000000         ; A = 0x80000000
    cpuid                       ; CPU identification
    cmp eax, 0x80000001         ; compare with 0x80000001
    jb .no_long_mode            ; Less, no long mode
    mov eax, 0x80000001         ; A = 0x80000001
    cpuid                       ; CPU Id
    test edx, 1 << 29           ; Test if LM-bit is set in D
    jz .no_long_mode            ; Not set, no long mode
    test edx, 1 << 20           ; Test if NX bit is set in D
    jz .no_NX                   ; Not set, no NX protection
    test edx, 1 << 11           ; Test if SYSCALL bit is set
    jz .no_SC                   ; Not set, no syscall instruction
    ret
.no_long_mode:
    mov al, "2"
    jmp _error
.no_NX:
    mov al, "3"
    jmp _error
.no_SC:
    mov al, "4"
    jmp _error
