#![feature(rustc_macro)]
#![feature(plugin)]
#![feature(const_fn)]
extern crate elfloader;
#[macro_use]
extern crate log;
extern crate paging;
extern crate constants;
extern crate uuid;
extern crate corepack;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate kernel_std;

use std::io::{Read, Write};
use std::fmt::Display;
use std::fs::File;
use std::collections::HashMap;

use elfloader::ElfBinary;
use elfloader::elf::{PF_X, PF_W};

use kernel_std::module::{Module, Text, Header};

use serde::Serialize;

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
    module: Module,
    id_map: HashMap<u64, Uuid>
}


impl elfloader::ElfLoader for ModuleWriter {
    fn allocate(&mut self, base: elfloader::VAddr, size: usize, flags: elfloader::elf::ProgFlag) {
        let id = Uuid::new_v4();
        debug!("New text {}", id.hyphenated());

        // create header structure
        let header = Header {
            id: id.clone(),
            base: Some(base as u64),
            size: size as u64,
            write: flags.0 & PF_W.0 == PF_W.0,
            execute: flags.0 & PF_X.0 == PF_X.0,
            depends: vec![]
        };

        // add it to our list
        self.module.headers.push(header);

        // add this id to the id_map
        self.id_map.insert(base as u64, id);
    }

    fn load(&mut self, base: elfloader::VAddr, region: &'static [u8]) {
        // allocate should be called bofer load
        let id = self.id_map.get(&(base as u64)).unwrap().clone();

        let text = Text {
            id: id,
            data: Vec::from(region)
        };

        self.module.texts.push(text);
    }
}

impl ModuleWriter {
    fn new() -> ModuleWriter {
        ModuleWriter {
            module: Module {
                magic: Uuid::parse_str("0af979b7-02c3-4ca6-b354-b709bec81199").unwrap(),
                id: Uuid::new_v4(),
                ports: vec![],
                headers: vec![],
                texts: vec![]
            },
            id_map: HashMap::new()
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

    // serialize everything

    // create the file
    let mut file = File::create(format!("{}/kernel.mod", MODULE_PREFIX))
        .expect("Failed to create output file");

    let mut ser = corepack::Serializer::new(|buf| {
        file.write_all(buf).expect("Failed to write output");
        Ok(())
    });

    loader.module.serialize(&mut ser).expect("Failed to serialize module");

    // done
}
