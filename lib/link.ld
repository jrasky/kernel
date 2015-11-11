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
OUTPUT_FORMAT("elf64-x86-64")
ENTRY(_start)

SECTIONS {
    /* start at one MiB */
    . = 1M + SIZEOF_HEADERS;

    /* multiboot2 header */
    .multiboot2 ALIGN(8) : {
        KEEP(target/multiboot2.o (.text))
    }

    /* first the multiboot header */
    .text ALIGN(4K) : {
        _kernel_top = .;
        target/kernel.o (.text)
    }

    /* read-only data */
    .rodata ALIGN(4K) : ALIGN(4K) {
        * (.rodata)
    }

    /* initialized, read-write data */
    .data ALIGN(4K) : ALIGN(4K) {
        * (.rodata)
        _kernel_end = .;
    }

    /* uninitialized, read-write data and stack */
    .bss ALIGN(4K) : {
        _bss = .;
        * (.bss)
    }

    /* create space for page tables */
    .pages ALIGN(4K) : {
        _p4_table = .;
        . += 4K;
        _p3_table = .;
        . += 4K;
        _p2_table = .;
        . += 4K;
    }

    /* create a stack */
    .stack ALIGN(4K) : {
        . += 4K;
        _stack_top = .;
    }

    /* other sections, which grub shouldn't zero out */
    .common ALIGN(4K) : {
        * (COMMON)
    }
}
