# Makefile
#
# Copyright (C) 2015 Jerome Rasky
#
# Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
# http://www.apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
# http://opensource.org/licenses/MIT>, at your option. This file may not be
# copied, modified, or distributed except according to those terms.

TARGET_DIR = ./target
LIB_DIR = ./lib
GEN_DIR = $(TARGET_DIR)/gen
ISO_DIR = $(TARGET_DIR)/iso

GRUB_CFG = $(LIB_DIR)/grub.cfg
GRUB_IMAGE = $(TARGET_DIR)/image.iso

ISO_GRUB_CFG = $(ISO_DIR)/boot/grub/grub.cfg
ISO_KERNEL = $(ISO_DIR)/boot/kernel.elf

STAGE1_KERNEL = $(TARGET_DIR)/libkernel.a
STAGE1_ASM = $(TARGET_DIR)/asm/long.o

STAGE2_BIN = $(TARGET_DIR)/stage2

GEN_ASM = $(GEN_DIR)/page_tables.asm
GEN_SOURCES = $(GEN_DIR)/page_tables.bin $(GEN_ASM)
GEN_OBJECTS = $(GEN_DIR)/page_tables.o

GLOBAL_TARGET_DIR = $(abspath $(TARGET_DIR))
export GLOBAL_TARGET_DIR

STAGE1 = $(TARGET_DIR)/stage1.elf
KERNEL = $(TARGET_DIR)/kernel.elf

STAGE1_LINK = $(LIB_DIR)/stage1.ld
STAGE1_OBJECTS = $(STAGE1_ASM) $(STAGE1_KERNEL)

KERNEL_LINK = $(LIB_DIR)/link.ld

BOOT_ASM = $(TARGET_DIR)/asm/bootstrap.o $(TARGET_DIR)/asm/long_boot.o $(TARGET_DIR)/asm/multiboot2.o

OBJECTS = $(GEN_OBJECTS) $(STAGE1_OBJECTS) $(BOOT_ASM)

LD_FLAGS = -n --gc-sections
AS_FLAGS = -f elf64
GRUB_RESCUE_FLAGS = -d /usr/lib/grub/i386-pc/

GRUB_RESCUE = grub-mkrescue
MKDIR = mkdir
RM = rm
LD = ld
AS = nasm
CP = cp

build: directories $(KERNEL)

image: directories $(GRUB_IMAGE)

$(GRUB_IMAGE): $(ISO_GRUB_CFG) $(ISO_KERNEL)
	$(GRUB_RESCUE) $(GRUB_RESCUE_FLAGS) -o $(GRUB_IMAGE) $(ISO_DIR)

directories: $(ISO_DIR)/boot/grub/ $(ISO_DIR) $(TARGET_DIR) $(GEN_DIR)

$(ISO_GRUB_CFG): $(GRUB_CFG)
	$(CP) $< $@

$(ISO_KERNEL): $(KERNEL)
	$(CP) $< $@

$(TARGET_DIR) $(GEN_DIR) $(ISO_DIR) $(ISO_DIR)/boot/grub/:
	$(MKDIR) -p $@

$(KERNEL): $(OBJECTS)
	$(LD) -T $(KERNEL_LINK) $(LD_FLAGS) -o $@ $^

$(STAGE1_KERNEL): kernel

$(STAGE1_ASM) $(BOOT_ASM): asm

$(STAGE2_BIN): stage2

$(GEN_SOURCES): $(STAGE2_BIN) $(STAGE1)
	$(STAGE2_BIN)

$(GEN_OBJECTS): $(GEN_SOURCES)
	$(AS) $(AS_FLAGS) -o $@ $(GEN_ASM)

asm kernel stage2:
ifeq ($(MAKECMDGOALS),image)
	$(MAKE) -C $@ build
else
	$(MAKE) -C $@ $(MAKECMDGOALS)
endif

$(STAGE1): $(STAGE1_OBJECTS)
	$(LD) -T $(STAGE1_LINK) $(LD_FLAGS) -o $@ $^

clean: asm kernel stage2
	$(RM) -rf $(TARGET_DIR)
	$(RM) -rf $(GEN_DIR)

.PHONY: build asm kernel stage2 image directories
