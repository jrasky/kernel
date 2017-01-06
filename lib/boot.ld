OUTPUT_FORMAT("elf32-i386")

ENTRY(_start)

SECTIONS {
    .boot 1M : ALIGN(8) {
        KEEP(asm/target/multiboot2.o (.multiboot2))
    }

    .text : ALIGN(4K) {
        * (.text*)
    }

    .rodata : ALIGN(4K) {
        * (.rodata*)
    }

    .data : ALIGN(4K) {
        * (.data*)
    }

    .bss : ALIGN(4K) {
        * (.bss*)
    }

	. = .;
    _boot_end = .;

    /DISCARD/ : {
        * (.gcc_except_table)
    }
}
