#![feature(custom_derive)]
#![feature(plugin)]
#![feature(const_fn)]
#![plugin(serde_macros)]
extern crate elfloader;
#[macro_use]
extern crate log;
extern crate paging;
extern crate constants;
extern crate uuid;
extern crate corepack;

use std::io::{Read, Write};
use std::fmt::Display;
use std::fs::File;
use std::collections::HashMap;

use std::cmp;

use elfloader::ElfBinary;
use elfloader::elf::{PF_X, PF_W, PF_R};

use uuid::Uuid;

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

struct ModuleWriter {
    files: HashMap<elfloader::VAddr, File>
}

// ModuleHeader heads the modules loaded by grub.
// The actual data follows on the next page boundary.
#[derive(Debug, Serialize, Deserialize)]
struct ModuleHeader {
    magic: [u8; 16], // 0af979b7-02c3-4ca6-b354-b709bec81199
    id: [u8; 16], // unique ID for this module
    base: u64, // base vaddr for this module
    // size is already provided by grub
    flags: u8 // write = 0x1, execute = 0x2
}

impl elfloader::ElfLoader for ModuleWriter {
    fn allocate(&mut self, base: elfloader::VAddr, size: usize, flags: elfloader::elf::ProgFlag) {
        let mut new_file = File::create(format!("kernel-{}.mod", Uuid::new_v4().urn()))
            .expect("Failed to open file");

        let mut module_flags = 0;

        if flags.0 & PF_X.0 {
            module_flags &= 0x2;
        }

        if flags.0 & PF_W.0 {
            module_flags &= 0x1;
        }

        // create header structure
        let header = ModuleHeader {
            magic: Uuid::parse_str("0af979b7-02c3-4ca6-b354-b709bec81199").unwrap().as_bytes(),
            id: Uuid::new_v4().as_bytes(),
            base: base as u64,
            flags: module_flags
        };

        // encode and write it to a file
        let mut bytes = corepack::to_bytes(header).expect("Failed to encode module header");
        let pad_len = align(bytes.len() as u64, 0x1000); // pad to page-align
        file.write_all(bytes).expect("Failed to write bytes to file");
        file.set_len(pad_len).expect("Failed to pad out file to page");

        self.files.insert(base, new_file);
    }

    fn load(&mut self, base: elfloader::VAddr, region: &'static [u8]) {
        let mut output = self.files.get_mut(&base).expect("Allocate not called before load");
        output.write_all(region).expect("Failed to write out region to file");
    }
}

impl ModuleWriter {
    fn new() -> ModuleWriter {
        ModuleWriter {
            files: HashMap::new()
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

    debug!("Writing out sections to files");
    let mut loader = ModuleWriter::new();
    elf.load(&mut loader);

    // done
}
