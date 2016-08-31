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

ENTRY(kernel_main)

SECTIONS {
    /* start at one MiB */
    /* don't allocate before the end of the kernel image at all */
    _image_begin = 0;

    /* dummy _boot_info, which long.asm expects */
    _boot_info = 0;
    _boot_info_size = 0;

    /* dummy _gen_segments_size and _gen_segments */
    _gen_segments_size = 0;
    _gen_max_paddr = 0;
    _gen_page_tables = 0;
    _gen_segments = 0;

    INCLUDE lib/high.ld

    _image_end = .;
}
