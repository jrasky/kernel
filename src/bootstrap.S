### bootstrap.S
###
### Copyright (C) 2015 Jerome Rasky
### 
### Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
### http://www.apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT
### or http://opensource.org/licenses/MIT>, at your option. This file may not be
### copied, modified, or distributed except according to those terms.

    ## Define entry point
    .section .text
    .global _start
    .type _start, @function
_start:
    ## in kernel!

    ## create an empty call frame
    xor %ebp, %ebp

    ## set up stack
    movl $_stack_top, %esp

    ## push stack info
    push %ebp
    mov %esp, %ebp

    ## push multiboot info
    push %ebx

    ## push multiboot magic
    push %eax

    ## call kmail
    call kmain

    ## remove arguments from frame
    add $5, %esp

    ## restore call frame
    pop %ebp

    ## hang
.hang:  
    cli
    hlt
    jmp .hang

    .size _start, . - _start
