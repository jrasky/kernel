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
INCLUDE lib/headers.ld

ENTRY(_start)

SECTIONS {
    /* start at one MiB */
    /* don't allocate before the end of the kernel image at all */
    _image_begin = 0;

    /* multiboot2 header */
    .boot ORIGIN(boot) : ALIGN(8) {
        KEEP(asm/target/multiboot2.o (.multiboot2))
        _boot_top = .;
        * (.boot_text)
        * (.boot_rodata)
        * (.boot_data)
        * (.gen_text)
        * (.gen_data)
        _boot_data = .;
        * (.boot_bss)
    } > boot :boot

    . = .;

    _boot_end = .;

    /* generated page tables */
    .pages ORIGIN(pages) : AT(ORIGIN(pages)) {
        KEEP(* (.gen_pages))
    } :boot

    INCLUDE lib/high.ld

    . = .;

    _image_end = .;
}
