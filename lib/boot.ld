OUTPUT_FORMAT("elf32-i386")

ENTRY(_start)

SECTIONS {
    .boot 1M : ALIGN(8) {
        KEEP(asm/target/multiboot2.o (.multiboot2))
        * (.text*)
        * (.rodata*)
        * (.data*)
        * (.bss*)
    }

    /DISCARD/ : {
        * (.gcc_except_table)
        * (.note*)
    }
}
