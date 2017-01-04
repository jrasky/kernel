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
    boot (ax)        : ORIGIN = 1M, LENGTH = 0x80000
    identity (a)     : ORIGIN = 2M, LENGTH = 9M
    text (ax)        : ORIGIN = 2M - 0x80000000, LENGTH = 2M
    rodata (ar ! xw) : ORIGIN = 4M - 0x80000000, LENGTH = 2M
    data (aw ! x)    : ORIGIN = 6M - 0x80000000, LENGTH = 2M
    bss (a ! ix)     : ORIGIN = 8M - 0x80000000, LENGTH = 2M
}

PHDRS {
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
        * (.rodata*)
    } > rodata AT> identity :rodata
    
    .note ALIGN(4K) : ALIGN(4K) {
        *(.note*)
    } > rodata AT> identity :rodata
    
    . = .;
    
    _rodata_end = .;
    
    /* initialized, read-write data and slab map */
    .data ORIGIN(data) : ALIGN(4K) {
        _data_top = .;
        * (.data*)
    } > data AT> identity :data
    
    . = .;
    
    _data_end = .;
    
    /* uninitialized, read-write data, slab, and stack */
    .bss ORIGIN(bss) : ALIGN(4K) {
        _bss_top = .;
        * (.bss*)
        * (COMMON)
    } >bss AT> identity :bss
    
    /DISCARD/ : {
        * (.gcc_except_table)
    }

    . = .;

    _image_end = .;
}
