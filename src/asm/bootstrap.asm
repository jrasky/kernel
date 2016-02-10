;;; bootstrap.S
;;;
;;; Copyright (C) 2015 Jerome Rasky
;;; 
;;; Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
;;; http://www.apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT
;;; or http://opensource.org/licenses/MIT>, at your option. This file may not be
;;; copied, modified, or distributed except according to those terms.

    ;; externs
    extern _boot_end
    extern _p3_table
    extern _p2_table
    extern _p4_table
    extern BOOT_INFO_ADDR
    extern _lboot

    ;; global
    global _start
    global _boot_info
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

    section .boot_data
_boot_info:
    dq 0

    ;; Define entry point
    section .boot_text
    bits 32
_start:
    ;; grub entry point

    ;; set up stack
    mov esp, _boot_end

    ;; save boot info
    mov dword [_boot_info], ebx

    ;; perform tests
    call _test_multiboot
    call _test_cpuid
    call _test_long_mode

    ;; set up long mode
    call _setup_page_tables
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
    ret
.no_long_mode:
    mov al, "2"
    jmp _error

_setup_page_tables:
    ;; map first P4 entry to P3 table
    mov eax, _p3_table
    or eax, 0b11                ; present + writable
    mov [_p4_table], eax

    ;; map first P3 entry to P2 table
    mov eax, _p2_table
    or eax, 0b11
    mov [_p3_table], eax

    ;; map each P2 entry to a huge 2MiB page
    mov ecx, 0

.map_p2_table:
    ;; map ecx-th P2 entry to a huge page table that starts at address 2MiB*ecx
    mov eax, 0x200000           ; 2MiB
    mul ecx                     ; start address of ecx-th
    or eax, 0b10000011          ; present + writable + huge
    mov [_p2_table + ecx * 8], eax ; map ecx-th entry

    inc ecx
    cmp ecx, 512
    jne .map_p2_table

    ret

_enable_paging:
    ;; load P4 to cr3 register
    mov eax, _p4_table
    mov cr3, eax

    ;; Enable PAE-flag in cr4
    mov eax, cr4
    or eax, 1 << 5
    mov cr4, eax

    ;; set the long mode bit in the EFER MSR
    mov ecx, 0xC0000080
    rdmsr
    or eax, 1 << 8
    wrmsr

    ;; enable paging in the cr0 register
    mov eax, cr0
    or eax, 1 << 31
    or eax, 1 << 16
    mov cr0, eax

    ret
