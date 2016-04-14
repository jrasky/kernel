	COM1 equ 0x3f8

    extern bootstrap

    global _start

	section .bss nobits
_boot_stack_end:
    resb 0xf000
_boot_stack:

    section .next exec

_start:
    ;; grub entry point

    ;; set up stack
    mov esp, _boot_stack

    ;; save eax
    push eax
    
    ;; set up serial
    call _setup_serial

    ;; restore eax
    pop eax

    ;; perform tests
    call _test_multiboot
    call _test_cpuid
    call _test_long_mode

    ;; call bootstrap code
    call bootstrap

    ;; should not return
    mov al, "B"
    jmp _error

;;; outputs 'Error: ' and the error code, then hangs
_error:
    push ax
    push "E"
    call _write_byte
    push "r"
    call _write_byte
    push "r"
    call _write_byte
    push "o"
    call _write_byte
    push "r"
    call _write_byte
    push ":"
    call _write_byte
    push " "
    call _write_byte
    ;; next byte is the error code
    call _write_byte
    ;; hang
_hang:
    cli
    hlt
    jmp _hang

_setup_serial:
    ;; disable all interrupts
    mov dx, COM1 + 1
    mov al, 0x00
    out dx, al

    ;; enable DLAB
    mov dx, COM1 + 3
    mov al, 0x80
    out dx, al

    ;; set divisor to 3 (38400 baud)
    mov dx, COM1 + 0
    mov al, 0x03
    out dx, al

    ;; high byte
    mov dx, COM1 + 1
    mov al, 0x00
    out dx, al

    ;; 8 bits, no parity, one stop bit
    mov dx, COM1 + 3
    mov al, 0x03
    out dx, al

    ;; Enable FIFO, clear them, with 14-byte threshold
    mov dx, COM1 + 2
    mov al, 0xc7
    out dx, al

    ;; IRQ enable, RTS/DSR set
    mov dx, COM1 + 4
    mov al, 0x0b
    out dx, al

    ret

_write_byte:
    xor ax, ax
    mov dx, COM1 + 5
.read:
    in al, dx
    and al, 0x20
    jz .read
    pop ax
    mov dx, COM1
    out dx, al
    ret

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
