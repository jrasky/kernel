	global _lboot

    extern _lstart

    section .boot_text
    bits 64
_lboot:
    ;; jump to actual entry
    jmp _lstart
