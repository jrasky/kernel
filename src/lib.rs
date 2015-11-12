#![feature(no_std, lang_items)]
#![no_std]
extern crate rlibc;

#[no_mangle]
pub extern fn kmain() {
    // kernel main

    let hello = b"Hello!";
    let color_byte = 0x1f; // white foreground, blue background

    let mut hello_colored = [color_byte; 12];
    for (i, char_byte) in hello.into_iter().enumerate() {
        hello_colored[i * 2] = *char_byte;
    }

    // write to the VGA text buffer
    let buffer_ptr = (0xb8000) as *mut _;
    unsafe {*buffer_ptr = hello_colored};

    loop {}
}

#[cfg(not(test))]
#[lang = "eh_personality"]
extern fn eh_personality() {}

#[cfg(not(test))]
#[lang = "panic_fmt"]
extern fn panic_fmt() -> ! {
    loop {}
}
