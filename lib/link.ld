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
    identity (a)   : ORIGIN = 1M, LENGTH = 9M
    text (ax)      : ORIGIN = 2M, LENGTH = 2M
    rodata (ar!xw) : ORIGIN = 4M, LENGTH = 2M
    data (aw!x)    : ORIGIN = 6M, LENGTH = 2M
    bss (a!ix)     : ORIGIN = 8M, LENGTH = 2M
}

PHDRS {
    headers PT_PHDR PHDRS FILEHDR;
    boot PT_LOAD FLAGS(5);
    text PT_LOAD FLAGS(5);
    rodata PT_LOAD FLAGS(4);
    data PT_LOAD FLAGS(6);
    bss PT_LOAD FLAGS(6);
}

SECTIONS {
    /* start at one MiB */
    /* don't allocate before the end of the kernel image at all */
    _image_begin = 0;

    /* multiboot2 header */
    .boot ORIGIN(identity) : ALIGN(8) {
        KEEP(target/asm/multiboot2.o (.multiboot2))
        _boot_top = .;
        * (.boot_text)
        * (.boot_rodata)
        * (.boot_data)
        . = ALIGN(4k);
        _p4_table = .;
        . += 4K;
        _p3_table = .;
        . += 4K;
        _p2_table = .;
        . += 4K;
        _boot_data = .;
        * (.boot_bss)
        . += 0x500; /* boot stack */
        _boot_end = .;
    } > identity :boot


    /* first the multiboot header */
    .text ORIGIN(text) : ALIGN(4K) {
        _kernel_top = .;
        * (.text*)
    } > text AT> identity :text

    . = .;

    _kernel_end = .;

    /* read-only data */
    .rodata ORIGIN(rodata) : ALIGN(4K) {
        _rodata_top = .;
        * (.gcc_except_table)
        * (.rodata*)
    } > rodata AT> identity :rodata

    . = .;

    _rodata_end = .;

    /* initialized, read-write data and slab map */
    .data ORIGIN(data) : ALIGN(4K) {
        _data_top = .;
        * (.data*)
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
    } > data AT> identity :data

    . = .;

    _data_end = .;

    /* global data page, for certain special kernel structures */
    .godata BLOCK(4K) : ALIGN(4K) {
        _godata_top = .;
        /* static place to shove the cr3 register for the core task */
        _core_pages = .;
        QUAD(0x0);
    } > data AT> identity

    . = .;

    _godata_end = .;

    /* uninitialized, read-write data, slab, and stack */
    .bss ORIGIN(bss) : ALIGN(4K) {
        _bss_top = .;
        * (.bss*)
        * (COMMON)
        _slab = .;
        . += 0x8000;
        /* create a stack */
        . += 0xf000;
    } >bss AT> identity :bss

    . = .;

    _stack_top = .;
    _image_end = .;
}
