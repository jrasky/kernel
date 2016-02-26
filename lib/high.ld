/* first the multiboot header */
.text ORIGIN(text) : ALIGN(4K) {
    _kernel_top = .;
    * (.text*)
} > text AT> identity :text

. = .;

_kernel_end = .;

/* read-only data */
.rodata ORIGIN(rodata) : ALIGN(4K) {
    _rodata_top = .;
    * (.rodata*)
} > rodata AT> identity :rodata

. = .;

_rodata_end = .;

/* initialized, read-write data and slab map */
.data ORIGIN(data) : ALIGN(4K) {
    _data_top = .;
    * (.data*)
} > data AT> identity :data

. = .;

_data_end = .;

/* uninitialized, read-write data, slab, and stack */
.bss ORIGIN(bss) : ALIGN(4K) {
    _bss_top = .;
    * (.bss*)
    * (COMMON)
} >bss AT> identity :bss

/DISCARD/ : {
    * (.gcc_except_table)
}
