OUTPUT_FORMAT("elf32-i386")

ENTRY(_start)

SECTIONS {
    .boot 1M : ALIGN(8) {
        KEEP(asm/target/multiboot2.o (.multiboot2))
        * (.text*)
    }

    .note.gnu.build-id : { *(.note.gnu.build-id) }

    .rodata ALIGN(8) : ALIGN(8) {
        * (.rodata*)
    }

    .data ALIGN(8) : ALIGN(8) {
        * (.data*)
    }

    .bss ALIGN(8) : ALIGN(8) {
        * (.bss*)
    }

    _end = .;

    /DISCARD/ : {
        * (.gcc_except_table)
    }
}

. = ASSERT((_end <= 0x200000), "boot image too big");
