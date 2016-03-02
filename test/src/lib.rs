#![feature(lang_items)]
#![feature(asm)]
#![feature(collections)]
#![no_std]

#[macro_use]
extern crate collections;
extern crate log;
extern crate user;

#[no_mangle]
pub extern "C" fn test_user_task() -> ! {
    let mut request = log::Request {
        level: 3,
        location: log::Location {
            module_path: module_path!(),
            file: file!(),
            line: line!()
        },
        target: module_path!().into(),
        message: "".into()
    };

    request.message = format!("Hello from a user task!");
    user::log(&request);

    for x2 in 0..5 {
        request.message = format!("x2: {}", x2);
        user::log(&request);
        user::release();
    }

    request.message = format!("User task done!");
    user::log(&request);
    user::exit();

}

#[cold]
#[inline(never)]
#[lang = "eh_personality"]
extern "C" fn eh_personality() {
    unreachable!("C++ exception code called")
}

#[cold]
#[lang = "panic_fmt"]
extern "C" fn panic_fmt() {
    loop {
        unsafe {
            asm!("cli; hlt" ::::);
        }
    }
}
