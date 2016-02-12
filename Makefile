# Makefile
#
# Copyright (C) 2015 Jerome Rasky
#
# Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
# http://www.apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
# http://opensource.org/licenses/MIT>, at your option. This file may not be
# copied, modified, or distributed except according to those terms.

BOOT_SOURCES = multiboot2.asm bootstrap.asm long_boot.asm

GEN_SOURCES = page_tables.bin page_tables.asm

STAGE2_SOURCES = stage2.rs

CORE_SOURCES = long.asm kernel.rs multiboot.rs memory/mod.rs memory/reserve.rs memory/simple.rs constants.rs error.rs logging.rs cpu/init/mod.rs cpu/init/gdt.rs cpu/init/idt.rs cpu/init/tss.rs cpu/interrupt.rs cpu/stack.rs cpu/task.rs cpu/syscall.rs

SOURCE_DIR = src
TARGET_DIR = target
LIB_DIR = lib
GEN_DIR = gen

LDFLAGS = --gc-sections
ARCH = x86_64-unknown-linux-gnu
RUSTFLAGS = 
GRUB_RESCUE_FLAGS = -d /usr/lib/grub/i386-pc/
STAGE2_FLAGS = --release

STAGE1 = $(TARGET_DIR)/stage1.elf
STAGE1_LINK = $(LIB_DIR)/stage1.ld
STAGE2_BIN = stage2
KERNEL = $(TARGET_DIR)/kernel.elf
LINK = $(LIB_DIR)/link.ld

GRUB_CFG = $(LIB_DIR)/grub.cfg
GRUB_IMAGE = $(TARGET_DIR)/image.iso
ISO_DIR = $(TARGET_DIR)/iso

ISO_GRUB_CFG = $(ISO_DIR)/boot/grub/grub.cfg
ISO_KERNEL = $(ISO_DIR)/boot/$(notdir $(KERNEL))

ASM_SOURCES = $(filter %.asm,$(CORE_SOURCES))
ASM_OBJECTS = $(ASM_SOURCES:%.asm=$(TARGET_DIR)/asm/%.o)
RUST_SOURCES = $(filter %.rs,$(CORE_SOURCES))
RUST_OBJECTS = $(TARGET_DIR)/$(ARCH)/debug/libkernel.a
STAGE2_RUST = $(STAGE2_SOURCES:%.rs=$(SOURCE_DIR)/bin/%.rs)
GEN_ASM = $(filter %.asm,$(GEN_SOURCES))
GENERATED = $(GEN_SOURCES:%=$(GEN_DIR)/%)
GEN_OBJECTS = $(GEN_ASM:%.asm=$(TARGET_DIR)/gen/%.o)
BOOT_OBJECTS = $(BOOT_SOURCES:%.asm=$(TARGET_DIR)/asm/%.o)
CORE_OBJECTS = $(ASM_OBJECTS) $(RUST_OBJECTS)
OBJECTS = $(GEN_OBJECTS) $(BOOT_OBJECTS) $(ASM_OBJECTS) $(C_OBJECTS) $(RUST_OBJECTS)

GRUB_RESCUE = grub-mkrescue
CC = gcc
LD = ld
AS = nasm
MKDIR = mkdir
CP = cp
RM = rm
QEMU_IMG = qemu-system-x86_64
QEMU_KERN = qemu-system-x86_64
CARGO = cargo

# default target
all: directories $(KERNEL)

# File targets

$(KERNEL): $(OBJECTS) $(LINK)
	$(LD) -n -T $(LINK) $(LDFLAGS) -o $@ $(filter-out $(LINK),$^)

$(STAGE1): $(CORE_OBJECTS) $(STAGE1_LINK)
	$(LD) -n -T $(STAGE1_LINK) $(LDFLAGS) -o $@ $(filter-out $(STAGE1_LINK),$^)

$(GENERATED): $(STAGE1) $(STAGE2_RUST)
	$(CARGO) run --bin $(STAGE2_BIN) $(STAGE2_FLAGS)

$(GEN_OBJECTS): $(TARGET_DIR)/gen/%.o : $(GEN_DIR)/%.asm
	$(AS) -f elf64 -o $@ $<

$(BOOT_OBJECTS): $(TARGET_DIR)/%.o : $(SOURCE_DIR)/%.asm
	$(AS) -f elf64 -o $@ $<

$(ASM_OBJECTS): $(TARGET_DIR)/%.o : $(SOURCE_DIR)/%.asm
	$(AS) -f elf64 -o $@ $<

$(RUST_OBJECTS): $(RUST_SOURCES:%=$(SOURCE_DIR)/%) Cargo.toml
	$(CARGO) rustc --target $(ARCH) -- $(RUSTFLAGS)

$(ISO_GRUB_CFG): $(GRUB_CFG)
	$(CP) $< $@

$(ISO_KERNEL): $(KERNEL)
	$(CP) $< $@

$(GRUB_IMAGE): $(ISO_GRUB_CFG) $(ISO_KERNEL)
	$(GRUB_RESCUE) $(GRUB_RESCUE_FLAGS) -o $(GRUB_IMAGE) $(ISO_DIR)

$(GEN_DIR) $(TARGET_DIR) $(TARGET_DIR)/asm $(TARGET_DIR)/rust $(TARGET_DIR)/gen $(dir $(ISO_GRUB_CFG)) $(dir $(ISO_KERNEL)) $(dir $(GRUB_IMAGE)):
	$(MKDIR) -p $@

# Phony targets

directories: $(TARGET_DIR) $(TARGET_DIR)/gen $(TARGET_DIR)/asm $(TARGET_DIR)/rust $(dir $(ISO_GRUB_CFG)) $(dir $(ISO_KERNEL)) $(dir $(GRUB_IMAGE)) $(GEN_DIR)

image: directories $(GRUB_IMAGE)

clean:
	$(RM) -r $(TARGET_DIR)
	$(RM) -r $(GEN_DIR)

run_img: image
	$(QEMU_IMG) -cdrom $(GRUB_IMAGE)

run_kern: $(KERNEL)
	$(QEMU_KERN) -kernel $(KERNEL)

.PHONY: image clean run_img run_kern directories all
