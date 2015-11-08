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

    ## Address tap
    .short 2, 0                 # type, flags
    .long 24                    # size
    .long multiboot2_header_top # header address
    .long _kernel_top           # start of kernel .text
    .long _kernel_end           # end of kernel .text
    .long _bss_end              # end of kernel .bss

    .align ALIGN

    ## Entry tag
    .short 3, 0                 # type, flags
    .long 12                    # size
    .long _start                # entry address

    .align ALIGN

    ## Framebuffer tag
    .short 5, 0                 # type, flags
    .long 20                    # size
    .long 640                   # width
    .long 480                   # width
    .long 32                    # bit depth

    .align ALIGN

    ## End tag
    .short 0, 0                 # type, flags
    .long 8

    ## end label
multiboot2_header_end:   
