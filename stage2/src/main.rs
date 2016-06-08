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

use std::io::{Read, Write, Seek, SeekFrom};
use std::fmt::Display;
use std::fs::File;
use std::collections::HashMap;

use elfloader::ElfBinary;
use elfloader::elf::{PF_X, PF_W};

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
#[derive(Serialize, Deserialize, Debug)]
struct ModuleHeader {
    magic: [u8; 16], // 0af979b7-02c3-4ca6-b354-b709bec81199
    id: [u8; 16], // unique ID for this module
    base: u64, // base vaddr for this module
    size: u64, // virtual memory size
    write: bool,
    execute: bool
}

impl elfloader::ElfLoader for ModuleWriter {
    fn allocate(&mut self, base: elfloader::VAddr, size: usize, flags: elfloader::elf::ProgFlag) {
        let id = Uuid::new_v4();
        debug!("New module kernel-{}.mod", id.hyphenated());

        let mut new_file = File::create(format!("{}/kernel-{}.mod", MODULE_PREFIX, id.hyphenated()))
            .expect("Failed to open file");

        // create header structure
        let header = ModuleHeader {
            magic: *Uuid::parse_str("0af979b7-02c3-4ca6-b354-b709bec81199").unwrap().as_bytes(),
            id: *id.as_bytes(),
            size: size as u64,
            base: base as u64,
            write: flags.0 & PF_W.0 == PF_W.0,
            execute: flags.0 & PF_X.0 == PF_X.0,
        };

        // encode and write it to a file
        let bytes = corepack::to_bytes(header).expect("Failed to encode module header");
        let pad_len = align(bytes.len() as u64, 0x1000); // pad to page-align
        new_file.write_all(bytes.as_slice()).expect("Failed to write bytes to file");
        new_file.set_len(pad_len).expect("Failed to pad out file to page");
        new_file.seek(SeekFrom::Start(pad_len)).expect("Failed to seek to end of file");

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
