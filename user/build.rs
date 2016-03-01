extern crate nasm_rs;

fn main() {
    nasm_rs::compile_library_args("libuser-asm.a", &["src/syscall.asm"], &["-Fdwarf"]);
}
