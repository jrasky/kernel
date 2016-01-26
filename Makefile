# Makefile
#
# Copyright (C) 2015 Jerome Rasky
#
# Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
# http://www.apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
# http://opensource.org/licenses/MIT>, at your option. This file may not be
# copied, modified, or distributed except according to those terms.

SOURCES = multiboot2.asm bootstrap.asm long.asm kernel.rs memory/mod.rs memory/reserve.rs memory/simple.rs constants.rs error.rs log/mod.rs log/vga.rs

SOURCE_DIR = src
TARGET_DIR = target
LIB_DIR = lib

CFLAGS = -fno-asynchronous-unwind-tables -ffreestanding -O2 -Wall -Wextra -Wpedantic
LDFLAGS = --gc-sections
ARCH = x86_64-unknown-linux-gnu
RUSTFLAGS = -C no-stack-check
GRUB_RESCUE_FLAGS = -d /usr/lib/grub/i386-pc/

KERNEL = $(TARGET_DIR)/kernel.elf
LINK = $(LIB_DIR)/link.ld

GRUB_CFG = $(LIB_DIR)/grub.cfg
GRUB_IMAGE = $(TARGET_DIR)/image.iso
ISO_DIR = $(TARGET_DIR)/iso

ISO_GRUB_CFG = $(ISO_DIR)/boot/grub/grub.cfg
ISO_KERNEL = $(ISO_DIR)/boot/$(notdir $(KERNEL))

ASM_SOURCES = $(filter %.asm,$(SOURCES))
ASM_OBJECTS = $(ASM_SOURCES:%.asm=$(TARGET_DIR)/asm/%.o)
C_SOURCES = $(filter %.c,$(SOURCES))
C_OBJECTS = $(C_SOURCES:%.c=$(TARGET_DIR)/c/%.o)
RUST_SOURCES = $(filter %.rs,$(SOURCES))
RUST_OBJECTS = $(TARGET_DIR)/$(ARCH)/debug/libkernel.a
OBJECTS = $(ASM_OBJECTS) $(C_OBJECTS) $(RUST_OBJECTS)

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
	$(LD) -n -T $(LINK) $(LDFLAGS) -o $@ $(filter-out $(TARGET_DIR) $(LINK),$^)

$(C_OBJECTS): $(TARGET_DIR)/%.o : $(SOURCE_DIR)/%.c
	$(CC) -m64 -c $(CFLAGS) -o $@ $<

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

$(TARGET_DIR):
	$(MKDIR) -p $@

$(TARGET_DIR)/asm:
	$(MKDIR) -p $@

$(TARGET_DIR)/c:
	$(MKDIR) -p $@

$(TARGET_DIR)/rust:
	$(MKDIR) -p $@

$(dir $(ISO_GRUB_CFG)):
	$(MKDIR) -p $@

$(dir $(ISO_KERNEL)):
	$(MKDIR) -p $@

$(dir $(GRUB_IMAGE)):
	$(MKDIR) -p $@

# Phony targets

directories: $(TARGET_DIR) $(TARGET_DIR)/asm $(TARGET_DIR)/c $(TARGET_DIR)/rust $(dir $(ISO_GRUB_CFG)) $(dir $(ISO_KERNEL)) $(dir $(GRUB_IMAGE))

image: directories $(GRUB_IMAGE)

clean:
	$(RM) -r $(TARGET_DIR)

run_img: image
	$(QEMU_IMG) -cdrom $(GRUB_IMAGE)

run_kern: $(KERNEL)
	$(QEMU_KERN) -kernel $(KERNEL)

.PHONY: image clean run_img run_kern directories all
