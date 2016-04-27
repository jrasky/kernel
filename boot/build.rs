extern crate gcc;

fn main() {
    gcc::Config::new()
        .file("src/boot_c.c")
        .flag("-ffreestanding")
        .compile("libboot_c.a");
}
