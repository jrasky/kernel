;; bootstrap.asm
; Constants
MBALIGN  equ 1<<0                ; align modules on page boundaries
MEMINFO  equ 1<<1                ; provide memory map
FLAGS    equ MBALIGN | MEMINFO   ; multiboot 'flag' field
MAGIC    equ 0x1BADB002          ; magic
CHECKSUM equ -(MAGIC + FLAGS)    ; checksum

; Declare a header for the start
section .multiboot
align 0x4
    dd MAGIC
    dd FLAGS
    dd CHECKSUM

; Stack point is currently random, change that
section .bootstrap_stack, nobits
align 4
stack_bottom:
resb 16384
stack_top:

; _start is multiboot entry point
section .text
global _start
_start:
    ; in kernel!

    ; set stack pointer
    mov esp, stack_top

    ; kernel main
    extern kmain
    call kmain

    ; if kernel returns, clear interrupts, then halt forever
    cli
.hang:
    hlt
    jmp .hang
