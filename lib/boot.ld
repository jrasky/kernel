OUTPUT_FORMAT("elf32-i386")

ENTRY(_start)

SECTIONS {
    .boot 1M : ALIGN(8) {
        KEEP(asm/target/multiboot2.o (.multiboot2))
        * (.text*)
    }

    .rodata ALIGN(8) : ALIGN(8) {
        * (.rodata*)
    }

    .data ALIGN(8) : ALIGN(8) {
        * (.data*)
    }

    .bss ALIGN(8) : ALIGN(8) {
        * (.bss*)
    }

    /DISCARD/ : {
        * (.gcc_except_table)
        * (.note*)
    }
}
