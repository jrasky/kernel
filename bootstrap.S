### bootstrap.S
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

    ## set up stack
    movl $_stack_top, %esp

    ## call kmail
    call kmain

    ## failsafe in case kernel returns
    cli
    hlt
.Lhang:
    jmp .Lhang

    .size _start, . - _start
