extern crate elfloader;
#[macro_use]
extern crate log;
extern crate paging;

use std::io::{Read, Write};
use std::fmt::Display;
use std::fs::File;

use std::slice;

use elfloader::ElfBinary;
use elfloader::elf::{PF_X, PF_W, PF_R};

pub const U64_BYTES: usize = 0x8;

pub const PAGE_TABLES_OFFSET: usize = 0x180000;

struct LogOutput;

impl log::Output for LogOutput {
    fn log(&mut self, level: usize, location: &log::Location, target: &Display, message: &Display) {
        if level <= 1 {
            println!("{} {} at {}({}): {}", target, log::level_name(level), location.file, location.line, message);
        } else {
            println!("{} {}: {}", target, log::level_name(level), message);
        }
    }
}

fn main() {
    log::set_output(Some(Box::new(LogOutput)));

    let mut layout = paging::Layout::new();

    debug!("Reading file");

    let mut file = File::open("target/stage1.elf").expect("Failed to open stage 1 file");

    let mut buf = vec![];

    file.read_to_end(&mut buf).expect("Failed to read file");

    let elf = ElfBinary::new("stage1.elf", buf.as_slice()).expect("Failed to parse stage 1 file");

    debug!("Geting program headers");

    for ref phdr in elf.program_headers() {
        if phdr.flags.0 & PF_R.0 == PF_R.0 {
            let segment = paging::Segment::new(
                phdr.paddr as usize, phdr.vaddr as usize, phdr.memsz as usize,
                phdr.flags.0 & PF_W.0 == PF_W.0,
                false,
                phdr.flags.0 & PF_X.0 == PF_X.0,
                false);

            trace!("Inserting segment: {:?}", segment);

            layout.insert(segment);
        }
    }

    debug!("Creating tables");

    let (address, tables) = layout.build_tables_relative(PAGE_TABLES_OFFSET);

    let bytes: &[u8] = unsafe {slice::from_raw_parts(tables.as_ptr() as *const _, tables.len() * U64_BYTES)};

    let mut raw_output = File::create("gen/page_tables.bin").expect("Failed to open raw output file");

    raw_output.write_all(bytes).expect("Failed to write bytes to output");

    let mut asm_output = File::create("gen/page_tables.asm").expect("Failed to open asm output file");

    writeln!(asm_output, concat!(
        "    global _gen_load_page_tables\n",

        "    section .gen_pages\n",
        "    incbin \"gen/page_tables.bin\"\n",

        "    section .gen_text\n",
        "    bits 32\n",
        "_gen_load_page_tables:\n",
        "    mov eax, 0x{:x}\n",
        "    mov cr3, eax\n",
        "    ret"), address).expect("Failed to write asm to output");

    info!("Created page tables at offset 0x{:x}", PAGE_TABLES_OFFSET);
}
