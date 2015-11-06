/*
 * link.ld
 */
ENTRY(_start)

SECTIONS {
    /* start at 1 MiB */
    . = 1M;

    /* first the multiboot header */
    .text ALIGN(4K) : {
    *(.multiboot)
    *(.text)
    }

    /* read-only data */
    .rodata BLOCK(4K) : ALIGN(4K) {
    *(.rodata)
    }

    /* initialized, read-write data */
    .data BLOCK(4K) : ALIGN(4K) {
    *(.rodata)
    }

    /* uninitialized, read-write data and stack */
    .bss BLOCK(4K) : ALIGN(4K) {
    *(COMMON)
    *(.bss)
    *(.bootstrap_stack)
    }
}
