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

    /* boot stub */
    .boot ORIGIN(boot) : ALIGN(4K) {
        asm/target/long_stub.o (.text)
    } > boot :boot

    INCLUDE lib/high.ld

    . = .;

    _image_end = .;
}
