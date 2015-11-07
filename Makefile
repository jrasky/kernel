# Makefile
#
# Copyright (C) 2015 Jerome Rasky
#
# Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
# http://www.apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
# http://opensource.org/licenses/MIT>, at your option. This file may not be
# copied, modified, or distributed except according to those terms.

ASM_SOURCES = multiboot.S multiboot2.S bootstrap.S
C_SOURCES = kernel.c

SOURCE_DIR = src
TARGET_DIR = target
LIB_DIR = lib

KERNEL = $(TARGET_DIR)/kernel.elf
LINK = $(LIB_DIR)/link.ld

GRUB_CFG = $(LIB_DIR)/grub.cfg
GRUB_IMAGE = $(TARGET_DIR)/image.iso
ISO_DIR = $(TARGET_DIR)/iso

ASM_OBJECTS = $(ASM_SOURCES:%.S=$(TARGET_DIR)/%.o)
C_OBJECTS = $(C_SOURCES:%.c=$(TARGET_DIR)/%.o)
OBJECTS = $(ASM_OBJECTS) $(C_OBJECTS)

CFLAGS = -ffreestanding -O2 -Wall -Wextra
LDFLAGS = -ffreestanding -O2 -nostdlib -lgcc

GRUB_RESCUE = grub2-mkrescue
CC = gcc
LD = gcc
AS = as
MKDIR = mkdir
CP = cp
RM = rm
QEMU_IMG = qemu-system-i386
QEMU_KERN = qemu-system-i386

$(KERNEL): $(OBJECTS)
	$(MKDIR) -p $(@D)
	$(LD) -m32 -T $(LINK) $(LDFLAGS) -o $@ $^

$(C_OBJECTS): $(TARGET_DIR)/%.o : $(SOURCE_DIR)/%.c
	$(MKDIR) -p $(@D)
	$(CC) -m32 -c $(CFLAGS) -o $@ $<

$(ASM_OBJECTS): $(TARGET_DIR)/%.o : $(SOURCE_DIR)/%.S
	$(MKDIR) -p $(@D)
	$(AS) --32 -o $@ $<

$(ISO_DIR)/boot/grub/grub.cfg: $(GRUB_CFG)
	$(MKDIR) -p $(@D)
	$(CP) $< $@

$(ISO_DIR)/boot/$(notdir $(KERNEL)): $(KERNEL)
	$(MKDIR) -p $(@D)
	$(CP) $< $@

$(GRUB_IMAGE): $(ISO_DIR)/boot/grub/grub.cfg $(ISO_DIR)/boot/$(notdir $(KERNEL))
	$(MKDIR) -p $(@D)
	$(GRUB_RESCUE) -o $(GRUB_IMAGE) $(ISO_DIR)

image: $(GRUB_IMAGE)

clean:
	$(RM) -r $(TARGET_DIR)

run_img: image
	$(QEMU_IMG) -cdrom $(GRUB_IMAGE)

run_kern: $(KERNEL)
	$(QEMU_KERN) -kernel $(KERNEL)

.PHONY: image clean run_img run_kern
