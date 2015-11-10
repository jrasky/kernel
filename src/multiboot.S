### multiboot.S
###
### Copyright (C) 2015 Jerome Rasky
###
### Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
### http://www.apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT
### or http://opensource.org/licenses/MIT>, at your option. This file may not be
### copied, modified, or distributed except according to those terms.

    ## Declare constants
    # Declare constants used for creating a multiboot header.
    .set ALIGN,    1<<0             # align loaded modules on page boundaries
    .set MEMINFO,  1<<1             # provide memory map
    .set VIDEO,    1<<2             # video mode info
    .set ENTRY,    1<<16            # entry point info
    .set FLAGS,    ALIGN | MEMINFO | VIDEO | ENTRY  # this is the Multiboot 'flag' field
    .set MAGIC,    0x1BADB002       # 'magic number' lets bootloader find the header
    .set CHECKSUM, -(MAGIC + FLAGS) # checksum of above, to prove we are multiboot

    ## Declare multiboot header
    .section .text
    .align ALIGN

multiboot_header_top:
    .long MAGIC
    .long FLAGS
    .long CHECKSUM

    ## Entry point info

    .long multiboot_header_top  # header magic address
    .long _kernel_top           # beginning of text
    .long _kernel_end           # end of data
    .long _stack_top            # end of bss + stack
    .long _start                # entry point

    ## Video mode info

    .long 1                     # EGA text mode
    .long 80                    # text columns
    .long 25                    # text lines
    .long 0                     # depth
