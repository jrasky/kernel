/*
 * link.ld
 *
 * Copyright (C) 2015 Jerome Rasky
 *
 * Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
 * http://www.apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT
 * or http://opensource.org/licenses/MIT>, at your option. This file may not be
 * copied, modified, or distributed except according to those terms.
 */
OUTPUT_FORMAT("elf32-i386")
ENTRY(_start)

SECTIONS {
    /* start at one MiB */
    . = 1M + SIZEOF_HEADERS;

    /* multiboot header */
    .multiboot ALIGN(4) : {
        target/multiboot.o (.text)
    }

    /* multiboot2 header */
    .multiboot2 ALIGN(8) : {
        target/multiboot2.o (.text)
    }

    /* align */
    . = ALIGN(4K);
    _kernel_top = .;

    /* first the multiboot header */
    .text : {
        target/kernel.o (.text)
    }

    /* read-only data */
    .rodata ALIGN(4K) : {
        * (.rodata)
    }

    /* initialized, read-write data */
    .data ALIGN(4K) : {
        * (.rodata)
    }

    _kernel_end = .;

    /* uninitialized, read-write data and stack */
    .bss ALIGN(4K) : {
        * (.bss)
    }

    /* create a stack */
    .stack ALIGN(4K) : {
        . = . + 0x4000;
    }

    /* tell the binaries about it */
    _stack_top = .;

    /* other sections, which grub shouldn't zero out */
    .common ALIGN(4K) : {
        * (COMMON)
    }
}
