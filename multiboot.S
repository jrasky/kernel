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
    .set FLAGS,    ALIGN | MEMINFO  # this is the Multiboot 'flag' field
    .set MAGIC,    0x1BADB002       # 'magic number' lets bootloader find the header
    .set CHECKSUM, -(MAGIC + FLAGS) # checksum of above, to prove we are multiboot

    ## Declare multiboot header
    .section .text
    .align ALIGN
    .long MAGIC
    .long FLAGS
    .long CHECKSUM
