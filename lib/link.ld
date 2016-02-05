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
    /* don't allocate before the end of the kernel image at all */
    _image_begin = 0;
    . = 1M + SIZEOF_HEADERS;

    /* multiboot2 header */
    .boot ALIGN(8) : {
        KEEP(target/asm/multiboot2.o (.multiboot2))
    }

    /* first the multiboot header */
    .text ALIGN(4K) : {
        _kernel_top = .;
        * (.text)
    }

    . = .;
    _kernel_end = .;

    /* read-only data */
    .rodata ALIGN(4K) : {
        _rodata_top = .;
        * (.rodata)
    }

    . = .;
    _rodata_end = .;

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
    }

    . = .;
    _data_end = .;

    /* global data page, for certain special kernel structures */
    .godata ALIGN(4K) : {
        _godata_top = .;
        /* static place to shove the cr3 register for the core task */
        _core_pages = .;
        . += 0x8;
    }

    . = .;
    _godata_end = .;

    /* uninitialized, read-write data, slab, and stack */
    .bss ALIGN(4K) : {
        _bss_top = .;
        * (.bss)
    }

    .slab ALIGN(4K) : {
        _slab = .;
        . += 0x8000;
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
        . += 0xf000;
    }

    . = .;
    _stack_top = .;

    /* other sections, which grub shouldn't zero out */
    .common ALIGN(4K) : {
        * (COMMON)
    }

    . = .;
    _image_end = .;
}
