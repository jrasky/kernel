;;; bootstrap.S
;;;
;;; Copyright (C) 2015 Jerome Rasky
;;; 
;;; Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
;;; http://www.apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT
;;; or http://opensource.org/licenses/MIT>, at your option. This file may not be
;;; copied, modified, or distributed except according to those terms.

    ;; externs
    extern _gen_load_page_tables
    extern _gen_max_paddr
    extern BOOT_INFO_ADDR
    extern _lboot

    ;; global
    global _start
    global _boot_info
    global _boot_info_size
    global _tss

    section .boot_rodata
gdt64:
    dq 0                        ;zero entry
.code: equ $ - gdt64    
    dq (1<<44) | (1<<47) | (1<<41) | (1<<43) | (1<<53) ;code segment
.data: equ $ - gdt64
    dq (1<<44) | (1<<47) | (1<<41)                     ;data segment
.pointer:
    dw $ - gdt64 - 1
    dq gdt64

    section .boot_bss nobits
_boot_info:
    resq 1
_boot_info_size:
    resq 1
_boot_stack_end:
    resb 0x500
_boot_stack:    

    ;; Define entry point
    section .boot_text exec
    bits 32
_start:
    ;; grub entry point

    ;; set up stack
    mov esp, _boot_stack

    ;; save boot info
    mov dword [_boot_info], ebx
    mov ecx, dword [ebx]
    mov dword [_boot_info_size], ecx

    ;; perform tests
    call _test_multiboot
    call _test_cpuid
    call _test_long_mode

    ;; set up long mode
    call _enable_paging

    ;; load the 64-bit GDT
    lgdt [gdt64.pointer]

    ;; update selectors
    mov ax, gdt64.data
    mov ss, ax                  ; stack selector
    mov ds, ax                  ; data selector
    mov es, ax                  ; extra selector

    ;; far jump to long mode
    jmp gdt64.code:_lboot       

    ;; hang
_hang:  
    cli
    hlt
    jmp _hang

    ;; Prints 'ERR: ' and an error code and hangs
_error:
    mov dword [0xB8000], 0x4F524F45
    mov dword [0xB8004], 0x4F3A4F52
    mov dword [0xB8008], 0x4F204F20
    mov byte  [0xB800A], al
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

_enable_paging:
    ;; load generated pages
    call _gen_load_page_tables

    ;; Enable PAE-flag, PSE-flag, and PGE-flag in cr4
    mov eax, cr4
    or eax, 0xb << 4
    mov cr4, eax

    ;; set the long mode bit and the NX bit in the EFER MSR
    mov ecx, 0xC0000080
    rdmsr
    or eax, 0x9 << 8
    ;; set the syscall bit in the EFER MSR
    or eax, 0x1
    wrmsr

    ;; enable paging in the cr0 register
    mov eax, cr0
    or eax, 1 << 31
    or eax, 1 << 16
    mov cr0, eax

    ret
