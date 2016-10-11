#![feature(proc_macro)]
#![feature(plugin)]
#![feature(const_fn)]
extern crate elf;
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

use kernel_std::module::{Module, Text, Header, Data, Placement, Port};

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

fn load_stage1() -> Module {
    info!("Loading stage1 binary");

    let elf_file = elf::File::open_path(STAGE1_ELF).expect("Failed to open stage1 binary");

    let mut file = File::open(STAGE1_ELF).expect("Failed to open stage1 file");

    let mut module = Module {
        magic: Uuid::parse_str("0af979b7-02c3-4ca6-b354-b709bec81199").unwrap(),
        id: Uuid::new_v4(),
        headers: vec![],
        texts: vec![]
    };

    debug!("Creating module {}", module.id);

    let entry = elf_file.ehdr.entry;

    debug!("File entry point at 0x{:x}", entry);

    // TODO: think about a more general tag structure for modules

    for program_header in elf_file.phdrs.iter() {
        if program_header.progtype.0 & elf::types::PT_LOAD.0 == elf::types::PT_LOAD.0 &&
            program_header.memsz > 0 && program_header.flags.0 & elf::types::PF_R.0 == elf::types::PF_R.0
        {
            // only include a header if it's loadable, has a non-zero size, and is readable

            if program_header.filesz > 0 && program_header.filesz != program_header.memsz {
                panic!("Don't know how to handle a partial program header");
            }

            let id = Uuid::new_v4();

            trace!("New header {} size 0x{:x}", id, program_header.memsz);

            let provides;

            // check to see if the entry point is in this text
            if program_header.vaddr <= entry && entry < program_header.vaddr + program_header.memsz {
                trace!("This text contains the entry point at offset 0x{:x}", entry - program_header.vaddr);
                provides = vec![Port {
                    id: Uuid::parse_str("b3de1342-4d70-449d-9752-3122338aa864").unwrap(),
                    offset: entry - program_header.vaddr
                }];
            } else {
                provides = vec![];
            }

            let header = Header {
                id: id,
                base: Placement::Absolute(program_header.vaddr),
                size: program_header.memsz,
                write: program_header.flags.0 & elf::types::PF_W.0 == elf::types::PF_W.0,
                execute: program_header.flags.0 & elf::types::PF_X.0 == elf::types::PF_X.0,
                provides: provides,
                requires: vec![]
            };

            // insert the header
            module.headers.push(header);

            if program_header.filesz == 0 {
                // record an empty text
                module.texts.push(Text {
                    id: id,
                    data: Data::Empty
                });
            } else {
                // read out the section we care about
                let mut buffer = vec![0; program_header.filesz as usize];
                file.seek(SeekFrom::Start(program_header.offset)).expect("Failed to seek");
                file.read_exact(buffer.as_mut_slice()).expect("Failed to read from file");
                module.texts.push(Text {
                    id: id,
                    data: Data::Direct(buffer)
                });
            }
        }
    }

    module
}

fn indirect_texts(module: &mut Module, texts: Vec<Text>) -> (Vec<Text>, Vec<u8>, u64) {
    info!("Figuring out offsets");

    // establish a lower bound for offsets

    let mut module_bytes = corepack::to_bytes(&module).expect("Failed to serialize module");

    // align the offset to a page boundary
    let mut offset = align(module_bytes.len() as u64, 0x1000);

    trace!("Initial offset at 0x{:x}", offset);

    loop {
        // first clear any texts we've placed into the module
        module.texts.clear();

        // update this offset as we account for each text
        let mut further_offset = offset as u64;

        for &Text { id, ref data } in texts.iter() {
            // align further_offset
            further_offset = align(further_offset, 0x1000);

            trace!("Trying text {} at offset 0x{:x}", id, further_offset);

            let new_text = Text {
                id: id,
                data: match data {
                    &Data::Direct(_) => Data::Offset(further_offset),
                    &Data::Empty => Data::Empty,
                    _ => unreachable!("All loader texts should be empty or direct")
                }
            };

            further_offset += match data {
                &Data::Direct(ref bytes) => bytes.len() as u64,
                &Data::Empty => 0,
                _ => unreachable!("All loader texts should be direct or empty")
            };

            module.texts.push(new_text);
        }

        // try serializing the module again
        module_bytes = corepack::to_bytes(&module).expect("Failed to serialize module");

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

    (texts, module_bytes, offset)
}

fn create_output(module: Module, texts: Vec<Text>, module_bytes: Vec<u8>, total_size: u64) {
    info!("Creating output");

    // serialize everything

    // create the file
    let mut file = File::create(format!("{}/kernel.mod", MODULE_PREFIX))
        .expect("Failed to create output file");

    // allocate the space we need
    file.set_len(total_size).expect("Failed to allocate space in kernel.mod");

    // write out module
    file.write_all(module_bytes.as_slice()).expect("Failed to write out module");

    // write out all our texts
    for (idx, text) in module.texts.iter().enumerate() {
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

        match texts[idx] {
            Text { id: _, data: Data::Direct(ref bytes) } => {
                file.write_all(bytes.as_slice()).expect("Failed to write out bytes");
            }
            Text { id: _, data: Data::Empty } => {
                // do nothing
            }
            _ => {
                unreachable!("All texts in loader should be direct or empty");
            }
        }
    }
}

fn main() {
    log::set_output(Some(Box::new(LogOutput)));

    let mut module = load_stage1();

    let texts = module.texts;
    module.texts = vec![];

    let (texts, module_bytes, total_size) = indirect_texts(&mut module, texts);

    create_output(module, texts, module_bytes, total_size);

    // done!
}
