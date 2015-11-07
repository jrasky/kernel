/*
 * link.ld
 */
OUTPUT_FORMAT("elf32-i386")
ENTRY(_start)

SECTIONS {
    /* start at one MiB */
    . = 1M + SIZEOF_HEADERS;

    /* multiboot header 
    .multiboot ALIGN(4) : {
        multiboot.o (.text)
    }*/

    /* multiboot2 header */
    .multiboot2 ALIGN(8) : {
        multiboot2.o (.text)
    }

    /* align */
    . = ALIGN(4K);
    _kernel_top = .;

    /* first the multiboot header */
    .text : {
        kernel.o (.text)
    }

    _kernel_end = .;

    /* read-only data */
    .rodata ALIGN(4K) : {
        * (.rodata)
    }

    /* initialized, read-write data */
    .data ALIGN(4K) : {
        * (.rodata)
    }

    /* uninitialized, read-write data and stack */
    .bss ALIGN(4K) : {
        *(COMMON)
        * (.bss)
    }

    _bss_end = .;

    /* create a stack */
    .stack ALIGN(4K) : {
        . = . + 0x4000;
    }

    /* tell the binaries about it */
    _stack_top = .;
}
