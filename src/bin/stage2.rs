extern crate elfloader;
#[macro_use]
extern crate log;
extern crate paging;

use std::io::Read;
use std::fmt::Display;
use std::fs::File;

use elfloader::ElfBinary;
use elfloader::elf::{PF_X, PF_W, PF_R};

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

    info!("Layout: {:?}", layout);
}
