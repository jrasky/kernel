#![feature(proc_macro)]
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

use std::io::{Read, Write, Seek, SeekFrom};
use std::fmt::Display;
use std::fs::File;
use std::collections::HashMap;

use elfloader::ElfBinary;
use elfloader::elf::{PF_X, PF_W};

use kernel_std::module::{Module, Text, Header, Data, Placement};

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
    data: HashMap<Uuid, Data>,
    id_map: HashMap<u64, Uuid>
}


impl elfloader::ElfLoader for ModuleWriter {
    fn allocate(&mut self, base: elfloader::VAddr, size: usize, flags: elfloader::elf::ProgFlag) {
        // ignore zero-size texts
        if size == 0 {
            return;
        }

        let id = Uuid::new_v4();
        trace!("New text {} size 0x{:x}", id.hyphenated(), size);

        // create header structure
        let header = Header {
            id: id.clone(),
            base: Placement::Absolute(base as u64),
            size: size as u64,
            write: flags.0 & PF_W.0 == PF_W.0,
            execute: flags.0 & PF_X.0 == PF_X.0,
            provides: vec![], // TODO: fill these in with whatever
            depends: vec![]
        };

        // add it to our list
        self.module.headers.push(header);

        // add this text to our data map
        assert!(self.data.insert(id.clone(), Data::Empty).is_none());

        // add this id to the id_map
        assert!(self.id_map.insert(base as u64, id).is_none(), "Two texts shared the same base address");
    }

    fn load(&mut self, base: elfloader::VAddr, region: &'static [u8]) {
        // ignore zero-size regions
        if region.len() == 0 {
            return;
        }

        // TODO: ensure data size matches the header size

        // allocate should be called before load
        let id = self.id_map.get(&(base as u64)).unwrap().clone();

        trace!("Loading region for text {} size 0x{:x}", id, region.len());

        // create a new data object
        let data = Data::Direct(Vec::from(region));

        // update our text data
        assert!(self.data.insert(id, data).is_some());
    }
}

impl ModuleWriter {
    fn new() -> ModuleWriter {
        ModuleWriter {
            module: Module {
                magic: Uuid::parse_str("0af979b7-02c3-4ca6-b354-b709bec81199").unwrap(),
                id: Uuid::new_v4(),
                headers: vec![],
                texts: vec![]
            },
            data: HashMap::new(),
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

    debug!("Loading binary");
    let mut loader = ModuleWriter::new();
    elf.load(&mut loader);

    debug!("Figuring out offsets");

    // establish a lower bound for offsets

    let mut module_bytes = corepack::to_bytes(&loader.module).expect("Failed to serialize module");

    // align the offset to a page boundary
    let mut offset = align(module_bytes.len() as u64, 0x1000);

    trace!("Initial offset at 0x{:x}", offset);

    loop {
        // first clear any texts we've placed into the module
        loader.module.texts.clear();

        // update this offset as we account for each text
        let mut further_offset = offset as u64;

        for (id, data) in loader.data.iter() {
            // align further_offset
            further_offset = align(further_offset, 0x1000);

            trace!("Trying text {} at offset 0x{:x}", id, further_offset);

            let new_text = Text {
                id: id.clone(),
                data: match data {
                    &Data::Direct(_) => Data::Offset(further_offset),
                    &Data::Empty => Data::Empty,
                    _ => unreachable!("ALl loader texts should be empty or direct")
                }
            };

            further_offset += match data {
                &Data::Direct(ref bytes) => bytes.len() as u64,
                &Data::Empty => 0,
                _ => unreachable!("All loader texts should be direct or empty")
            };

            loader.module.texts.push(new_text);
        }

        // try serializing the module again
        module_bytes = corepack::to_bytes(&loader.module).expect("Failed to serialize module");

        trace!("Module now 0x{:x} bytes long", module_bytes.len());

        if module_bytes.len() as u64 <= offset {
            // we're done!

            // set offset to further_offset so we can access it outside this loop
            offset = further_offset;
            break;
        } else {
            // try again
            offset = align(module_bytes.len() as u64, 0x1000);
        }
    }

    debug!("Creating output");

    // serialize everything

    // create the file
    let mut file = File::create(format!("{}/kernel.mod", MODULE_PREFIX))
        .expect("Failed to create output file");

    // allocate the space we need
    file.set_len(offset).expect("Failed to allocate space in kernel.mod");

    // write out module
    file.write_all(module_bytes.as_slice()).expect("Failed to write out module");

    // write out all our texts
    for text in loader.module.texts.iter() {
        let offset = match text.data {
            Data::Offset(addr) => addr,
            Data::Empty => {
                trace!("Text {} is empty", text.id);
                continue;
            }
            _ => unreachable!("All texts in loader module should be offsets")
        };

        trace!("Writing text {} at 0x{:x}", text.id, offset);

        file.seek(SeekFrom::Start(offset)).expect("Failed to seek in file");

        match loader.data.get(&text.id).unwrap() {
            &Data::Direct(ref bytes) => {
                file.write_all(bytes.as_slice()).expect("Failed to write out bytes");
            }
            &Data::Empty => {
                // do nothing
            }
            _ => {
                unreachable!("All texts in loader should be direct or empty");
            }
        }
    }

    // done!
}
