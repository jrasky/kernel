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

MEMORY {
    boot_info (r!xw)     : ORIGIN = 1M, LENGTH = 1M
    text_memory (x)      : ORIGIN = 2M, LENGTH = 2M
    rodata_memory (r!xw) : ORIGIN = 4M, LENGTH = 2M
    data_memory (w!x)    : ORIGIN = 6M, LENGTH = 2M
    bss_memory (!ix)     : ORIGIN = 8M, LENGTH = 2M
}

SECTIONS {
    /* start at one MiB */
    /* don't allocate before the end of the kernel image at all */
    _image_begin = 0;
    . = ORIGIN(boot_info) + SIZEOF_HEADERS;

    /* multiboot2 header */
    .boot ALIGN(8) : {
        KEEP(target/asm/multiboot2.o (.multiboot2))
    } > boot_info

    . = ORIGIN(text_memory);

    /* first the multiboot header */
    .text ALIGN(4K) : {
        _kernel_top = .;
        * (.text)
    } > text_memory

    . = .;
    _kernel_end = .;

    . = ORIGIN(rodata_memory);

    /* read-only data */
    .rodata ALIGN(4K) : {
        _rodata_top = .;
        * (.rodata)
    } > rodata_memory

    . = .;
    _rodata_end = .;

    . = ORIGIN(data_memory);

    /* initialized, read-write data and slab map */
    .data ALIGN(4K) : {
        _data_top = .;
        * (.data)
        _slab_map = .;
        /* slab map size = slab size / 8 */
        . += 0x1000;
        /* fxsave area, which needs to be aligned to 16 bytes */
        . = ALIGN(16);
        _fxsave_int = .;
        . += 0x200;
        . = ALIGN(16);
        _fxsave_task = .;
        . += 0x200;
    } > data_memory

    /* create space for page tables */
    .pages ALIGN(4K) : {
        _p4_table = .;
        . += 4K;
        _p3_table = .;
        . += 4K;
        _p2_table = .;
        . += 4K;
    } > data_memory

    . = .;
    _data_end = .;

    /* global data page, for certain special kernel structures */
    .godata ALIGN(4K) : {
        _godata_top = .;
        /* static place to shove the cr3 register for the core task */
        _core_pages = .;
        . += 0x8;
    } > data_memory

    . = .;
    _godata_end = .;

    . = ORIGIN(bss_memory);

    /* uninitialized, read-write data, slab, and stack */
    .bss ALIGN(4K) : {
        _bss_top = .;
        * (.bss)
    } > bss_memory

    .slab ALIGN(4K) : {
        _slab = .;
        . += 0x8000;
    } > bss_memory

    /* create a stack */
    .stack ALIGN(4K) : {
        . += 0xf000;
    } > bss_memory

    . = .;
    _stack_top = .;

    /* other sections, which grub shouldn't zero out */
    .common ALIGN(4K) : {
        * (COMMON)
    } > bss_memory

    . = .;
    _image_end = .;
}
