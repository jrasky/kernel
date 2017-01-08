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

use std::mem;

use kernel_std::module::{Module, Text, Data, Placement, Port, Type, Partition};

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

    let elf_file = elf::File::open_path(KERNEL_ELF).expect("Failed to open stage1 binary");

    let mut file = File::open(KERNEL_ELF).expect("Failed to open stage1 file");

    let mut module = Module {
        magic: Uuid::parse_str("0af979b7-02c3-4ca6-b354-b709bec81199").unwrap(),
        identity: Uuid::new_v4(),
        size: 0, // this will be filled in later
        partitions: vec![],
        texts: vec![]
    };

    debug!("Creating module {}", module.identity);

    let entry = elf_file.ehdr.entry;

    debug!("File entry point at 0x{:x}", entry);

    let mut id = 0u64;

    // TODO: think about a more general tag structure for modules

    for program_header in elf_file.phdrs.iter() {
        if program_header.progtype.0 & elf::types::PT_LOAD.0 == elf::types::PT_LOAD.0 &&
            program_header.memsz > 0 && program_header.flags.0 & elf::types::PF_R.0 == elf::types::PF_R.0
        {
            // only include a header if it's loadable, has a non-zero size, and is readable

            if program_header.filesz > 0 && program_header.filesz != program_header.memsz {
                panic!("Don't know how to handle a partial program header");
            }

            trace!("New header {} size 0x{:x}", id, program_header.memsz);

            let provides;

            // check to see if the entry point is in this text
            if program_header.vaddr <= entry && entry < program_header.vaddr + program_header.memsz {
                trace!("This text contains the entry point at offset 0x{:x}", entry - program_header.vaddr);
                provides = vec![Port {
                    identity: Uuid::parse_str("b3de1342-4d70-449d-9752-3122338aa864").unwrap(),
                    offset: entry - program_header.vaddr
                }];
            } else {
                provides = vec![];
            }

            let write = program_header.flags.0 & elf::types::PF_W.0 == elf::types::PF_W.0;
            let execute = program_header.flags.0 & elf::types::PF_X.0 == elf::types::PF_X.0;

            let data = if program_header.filesz == 0 {
                // no data provided
                trace!("Empty header");
                Data::Empty
            } else {
                // read out the section we care about
                trace!("Header with data");
                let mut buffer = vec![0; program_header.filesz as usize];
                file.seek(SeekFrom::Start(program_header.offset)).expect("Failed to seek");
                file.read_exact(buffer.as_mut_slice()).expect("Failed to read from file");
                Data::Direct(buffer)
            };

            let text = Text {
                id: id,
                base: Placement::Absolute(program_header.vaddr),
                size: program_header.memsz,
                ty: if execute { Type::Code } else { Type::Data { write: write } },
                provides: provides,
                requires: vec![],
                exports: vec![],
                imports: vec![],
                data: data
            };

            // add the text
            module.texts.push(text);

            // increment id
            id += 1;
        }
    }

    module
}

fn write_output(mut module: Module) {
    info!("Moving texts to partitions");

    // create space for the data we're going to write to the file later
    let mut data: Vec<Vec<u8>> = vec![];

    for text in module.texts.iter_mut() {
        if let Data::Empty = text.data {
            // nothing to do in this case
            continue;
        }

        // use data.len() as a shortcut for our own index before we insert ourselves
        let index = module.partitions.len() as u64;

        // do this dance to avoid copying things around
        debug!("Creating partition {}", index);

        // create the new data
        let mut new_data = Data::Offset { partition: index, offset: 0 };

        // swap out the data in the text
        mem::swap(&mut new_data, &mut text.data);

        // create a partition for this text
        if let Data::Direct(bytes) = new_data {
            // add the partition
            module.partitions.push(Partition {
                index: index,
                align: 0x1000,
                size: bytes.len() as u64
            });

            // add the bytes to an acumulator so we can write them out later
            data.push(bytes);

            // Data::Empty case treated at the top of the loop
        } else {
            unreachable!("Data in module was not Direct");
        }
    }

    info!("Creating output");

    // serialize the module
    let mut module_bytes;

    loop {
        module_bytes = corepack::to_bytes(&module).expect("Failed to serialize module");

        let new_module_size = module_bytes.len() as u64;

        if new_module_size == module.size {
            break;
        }

        module.size = new_module_size;
    }

    // create the file
    let mut file = File::create(KERNEL_MOD).expect("Failed to create output file");

    // write out module
    file.write_all(module_bytes.as_slice()).expect("Failed to write out module");

    let mut offset: u64 = align(module.size, 0x1000);

    file.seek(SeekFrom::Start(offset)).expect("Failed to seek");

    // write all the partitions
    for data in data.iter() {
        // TODO: find a more robust way to write these out
        offset = align(offset, 0x1000);
        trace!("Writing partition data at 0x{:x}", offset);
        file.seek(SeekFrom::Start(offset)).expect("Failed to seek file");
        file.write_all(data.as_slice()).expect("Failed to write data");
        offset += data.len() as u64;
    }
}

fn main() {
    log::set_output(Some(Box::new(LogOutput)));

    let module = load_stage1();

    write_output(module);

    // done!
}
