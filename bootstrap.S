### bootstrap.S
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
