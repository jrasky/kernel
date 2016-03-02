OUTPUT_FORMAT("elf64-x86-64")

MEMORY {
    boot (ax)        : ORIGIN = 1M, LENGTH = 0x80000
    pages (a)        : ORIGIN = 0x180000, LENGTH = 1M - 0x180000
    identity (a)     : ORIGIN = 2M, LENGTH = 9M
    text (ax)        : ORIGIN = 2M - 0x80000000, LENGTH = 2M
    rodata (ar ! xw) : ORIGIN = 4M - 0x80000000, LENGTH = 2M
    data (aw ! x)    : ORIGIN = 6M - 0x80000000, LENGTH = 2M
    bss (a ! ix)     : ORIGIN = 8M - 0x80000000, LENGTH = 2M

    user_text        : ORIGIN = 4M, LENGTH = 2M
    user_data        : ORIGIN = 0x606000, LENGTH = 2M - 0x6000
}

PHDRS {
    boot PT_LOAD FLAGS(5);
    text PT_LOAD FLAGS(5);
    rodata PT_LOAD FLAGS(4);
    data PT_LOAD FLAGS(6);
    bss PT_LOAD FLAGS(6);
    user_text PT_LOAD FLAGS(5);
    user_data PT_LOAD FLAGS(6);
}
