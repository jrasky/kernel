#![feature(const_fn)]
extern crate elfloader;
#[macro_use]
extern crate log;
extern crate paging;
extern crate constants;

use std::io::{Read, Write};
use std::fmt::Display;
use std::fs::File;

use std::slice;
use std::cmp;

use elfloader::ElfBinary;
use elfloader::elf::{PF_X, PF_W, PF_R};

use constants::*;

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

    debug!("Creating base layout");

    // base layout means the first two megabytes identity-mapped

    let mut layout = paging::Layout::new();
    let mut segments = vec![
        paging::Segment::new(
            0x0, 0x0, 0x200000,
            true, false, true, false
        )];

    assert!(layout.insert(paging::Segment::new(
        0x0, 0x0, 0x200000,
        true, false, true, false
    )), "Failed to insert segment");

    debug!("Reading file");

    let mut file = File::open(STAGE1_ELF).expect("Failed to open stage 1 file");

    let mut buf = vec![];

    file.read_to_end(&mut buf).expect("Failed to read file");

    let elf = ElfBinary::new("stage1.elf", buf.as_slice()).expect("Failed to parse stage 1 file");

    debug!("Geting program headers");

    let mut max_paddr = 0x200000;

    for ref phdr in elf.program_headers() {
        if phdr.flags.0 & PF_R.0 == PF_R.0 && phdr.memsz > 0 {
            let segment = paging::Segment::new(
                phdr.paddr as usize, phdr.vaddr as usize, phdr.memsz as usize,
                phdr.flags.0 & PF_W.0 == PF_W.0,
                false, // user
                phdr.flags.0 & PF_X.0 == PF_X.0,
                false); // global

            trace!("Inserting segment: {:?}", segment);

            max_paddr = cmp::max(max_paddr, align(phdr.paddr as usize + phdr.memsz as usize, 0x200000));

            segments.push(segment.clone());

            assert!(layout.insert(segment), "Failed to insert segment");
        }
    }

    trace!("Max paddr: 0x{:x}", max_paddr);

    // optimistically insert a 2-MB segment immediately following max_paddr
    let segment = paging::Segment::new(
        max_paddr, HEAP_BEGIN, 0x200000,
        true, // write
        false, // user
        false, // execute
        false, // global
    );

    trace!("Inserting segment: {:?}", segment);
    segments.push(segment.clone());
    assert!(layout.insert(segment), "Failed to insert heap segment");

    debug!("Creating tables");

    let (address, tables) = layout.build_tables_relative(PAGE_TABLES_OFFSET);

    trace!("Giant table address: 0x{:x}", address);

    debug!("Getting raw tables");

    let mut segment_buffer: Vec<u8> = vec![];

    for segment in segments.iter() {
        segment_buffer.extend(segment.get_raw().into_iter());
    }

    debug!("Writing output");

    let bytes: &[u8] = unsafe {slice::from_raw_parts(tables.as_ptr() as *const _, tables.len() * U64_BYTES)};

    let mut raw_output = File::create(RAW_OUTPUT).expect("Failed to open raw output file");

    raw_output.write_all(bytes).expect("Failed to write bytes to output");

    let mut seg_output = File::create(SEG_OUTPUT).expect("Failed to open seg output file");

    seg_output.write_all(segment_buffer.as_slice()).expect("Failed to write segments to output");

    let mut asm_output = File::create(ASM_OUTPUT).expect("Failed to open asm output file");

    write!(asm_output, concat!(
        "    global _gen_load_page_tables\n",
        "    global _gen_segments_size\n",
        "    global _gen_max_paddr\n",
        "    global _gen_page_tables\n",
        "    global _gen_segments\n",

        "    section .gen_pages\n",
        "    incbin \"{}\"\n",

        "    section .gen_data\n",
        "_gen_segments_size:\n",
        "    dq {}\n",
        "_gen_max_paddr:\n",
        "    dq {}\n",
        "_gen_page_tables:\n",
        "    dq {}\n",
        "_gen_segments:\n",
        "    incbin \"{}\"\n",

        "    section .gen_text\n",
        "    bits 32\n",
        "_gen_load_page_tables:\n",
        "    mov eax, 0x{:x}\n",
        "    mov cr3, eax\n",
        "    ret\n"),
             RAW_OUTPUT, segments.len(), max_paddr, address, SEG_OUTPUT, address)
        .expect("Failed to write asm to output");

    info!("Created page tables at offset 0x{:x}", PAGE_TABLES_OFFSET);
}
