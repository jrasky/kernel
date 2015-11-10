### multiboot2.S
###
### Copyright (C) 2015 Jerome Rasky
### 
### Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
### http://www.apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT
### or http://opensource.org/licenses/MIT>, at your option. This file may not be
### copied, modified, or distributed except according to those terms.
    
    ## Declare constants
    .set ALIGN, 8
    .set MAGIC, 0xE85250D6
    .set ARCH, 0
    .set LENGTH, multiboot2_header_end - multiboot2_header_top
    .set CHECKSUM, -(MAGIC + ARCH + LENGTH)

    .section .text
    .align ALIGN

    ## top label
multiboot2_header_top:

    ## header
    .long MAGIC
    .long ARCH
    .long LENGTH
    .long CHECKSUM

    .align ALIGN

    ## Info request tag
    .short 1, 0
    .long 16                     # size
    .long 8                      # framebuffer info
    .long 0                      # end info requests

    .align ALIGN

    ## Address tag
    .short 2, 0                 # type, flags
    .long 24                    # size
    .long multiboot2_header_top # header address
    .long _kernel_top           # start of kernel .text
    .long _kernel_end           # end of kernel .text
    .long _stack_top            # end of uninitialized data, including stack

    .align ALIGN

    ## Entry tag
    .short 3, 0                 # type, flags
    .long 12                    # size
    .long _start                # entry address

    .align ALIGN

    ## Flags tag
    .short 4, 0
    .long 12
    .long 3

    .align ALIGN

    ## End tag
    .short 0, 0                 # type, flags
    .long 8

    ## end label
multiboot2_header_end:   
