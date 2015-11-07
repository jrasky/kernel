### multiboot2.S
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

    ## Flags tag
    .short 4, 0                 # type, flags
    .long 12                    # size
    .long 3                     # console_flags

    .align ALIGN

    ## End tag
    .short 0, 0                 # type, flags
    .long 8

    ## end label
multiboot2_header_end:   
