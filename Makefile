## Rust source directories
BOOT_DIRS = ./boot ./log ./log_abi ./kernel_std ./serial ./constants ./paging ./memory
KERNEL_DIRS = ./kernel ./log ./log_abi ./paging ./user ./constants ./serial ./memory ./kernel_std
STAGE2_DIRS = ./stage2 ./paging ./constants ./kernel_std ./log ./log_abi

## Other build directories

ASM_DIR = ./asm
LIB_DIR = ./lib
TARGET_DIR = ./target

ISO_DIR = $(TARGET_DIR)/iso

## Lists of all the source files for each target

KERNEL_SOURCES = $(foreach dir,$(KERNEL_DIRS),$(shell find $(dir)/src/ -type f -name '*.rs') $(dir)/Cargo.toml) ./kernel/Cargo.lock
BOOT_SOURCES = $(foreach dir,$(BOOT_DIRS),$(shell find $(dir)/src/ -type f -name '*.rs') $(dir)/Cargo.toml) ./boot/Cargo.lock
STAGE2_SOURCES = $(foreach dir,$(STAGE2_DIRS),$(shell find $(dir)/src/ -type f -name '*.rs') $(dir)/Cargo.toml) ./stage2/Cargo.lock
ASM_SOURCES = $(wildcard $(ASM_DIR)/src/*)

## Boot target info

BOOT = $(TARGET_DIR)/boot.elf
BOOT_TARGET = ./boot/target/i686-unknown-linux-gnu/debug/libboot.a
BOOT_LINK = $(LIB_DIR)/boot.ld
BOOT_ASM = $(ASM_DIR)/target/multiboot2.o $(ASM_DIR)/target/short_boot.o

## Kernel target info

KERNEL = $(TARGET_DIR)/kernel.elf
KERNEL_MOD = $(TARGET_DIR)/kernel.mod
KERNEL_TARGET = ./kernel/target/debug/libkernel.a
KERNEL_LINK = $(LIB_DIR)/link.ld
KERNEL_ASM = $(ASM_DIR)/target/util.o

## Stage2 target info

STAGE2 = ./stage2/target/debug/stage2

## Grub source and output files

GRUB_CFG = $(LIB_DIR)/grub.cfg
GRUB_IMAGE = $(TARGET_DIR)/image.iso

## Locations of output in the image

ISO_GRUB_CFG = $(ISO_DIR)/boot/grub/grub.cfg
ISO_KERNEL = $(ISO_DIR)/boot/kernel.mod
ISO_BOOT = $(ISO_DIR)/boot/boot.elf

## Assembler and linker flags for the kernel

KERNEL_ASFLAGS = -f elf64
KERNEL_LDFLAGS = --gc-sections -n

## Assembler and linker flags for bootstrap

BOOT_ASFLAGS = -f elf32
BOOT_LDFLAGS = --gc-sections -n -m elf_i386

## Flags for other utilities

GRUB_RESCUE_FLAGS = -d /usr/lib/grub/x86_64-efi/
VM_FLAGS = -enable-kvm -net none -m 1024 -drive file=/usr/share/ovmf/ovmf_x64.bin,format=raw,if=pflash,readonly -k en-us -serial stdio -d cpu_reset,unimp,guest_errors
VM_DEBUG_FLAGS = $(VM_FLAGS) -s -S

## Commands to use

CD = cd
CARGO = cargo
AS = nasm
LD = ld
MKDIR = mkdir
RM = rm
GRUB_RESCUE = grub-mkrescue
CP = cp
VM = qemu-system-x86_64

## Phony targets

build: directories $(GRUB_IMAGE)

run: build
	$(VM) $(VM_FLAGS) -cdrom $(GRUB_IMAGE)

debug: build
	$(VM) $(VM_DEBUG_FLAGS) -cdrom $(GRUB_IMAGE)

clean:
	$(RM) -rf $(TARGET_DIR) $(ASM_DIR)/target
	$(CD) ./stage2 && cargo clean
	$(CD) ./kernel && cargo clean
	$(CD) ./boot && cargo clean

directories: $(TARGET_DIR) $(ASM_DIR)/target $(ISO_DIR) $(ISO_DIR)/boot $(ISO_DIR)/boot/grub

# creates the directories that we need for building
$(TARGET_DIR) $(ASM_DIR)/target $(ISO_DIR) $(ISO_DIR)/boot $(ISO_DIR)/boot/grub:
	$(MKDIR) -p $@

## Grub image targets

$(GRUB_IMAGE): $(ISO_GRUB_CFG) $(ISO_KERNEL) $(ISO_BOOT)
	$(GRUB_RESCUE) $(GRUB_RESCUE_FLAGS) -o $(GRUB_IMAGE) $(ISO_DIR)

$(ISO_GRUB_CFG): $(GRUB_CFG)
	$(CP) $< $@

$(ISO_KERNEL): $(KERNEL_MOD)
	$(CP) $< $@

$(ISO_BOOT): $(BOOT)
	$(CP) $< $@

# The kernel gets re-packed by stage2 so boot can load it

$(KERNEL_MOD): $(KERNEL) $(STAGE2)
	$(STAGE2)

## Linker targets

# Stage2's binary is linked by cargo

$(BOOT): $(BOOT_TARGET) $(BOOT_ASM) $(BOOT_LINK)
	$(LD) $(BOOT_LDFLAGS) -T $(BOOT_LINK) -o $@ $(BOOT_ASM) $(BOOT_TARGET)

$(KERNEL): $(KERNEL_TARGET) $(KERNEL_ASM) $(KERNEL_LINK)
	$(LD) $(KERNEL_LDFLAGS) -T $(KERNEL_LINK) -o $@ $(KERNEL_ASM) $(KERNEL_TARGET)

## Rust targets

$(STAGE2): $(STAGE2_SOURCES)
	$(CD) ./stage2 && $(CARGO) build

$(BOOT_TARGET): $(BOOT_SOURCES)
	$(CD) ./boot && $(CARGO) build --target i686-unknown-linux-gnu

$(KERNEL_TARGET): $(KERNEL_SOURCES)
	$(CD) ./kernel && $(CARGO) build

## Assembly targets

$(KERNEL_ASM): $(ASM_DIR)/target/%.o : $(ASM_DIR)/src/%.asm
	$(AS) $(KERNEL_ASFLAGS) -o $@ $<

$(BOOT_ASM): $(ASM_DIR)/target/%.o : $(ASM_DIR)/src/%.asm
	$(AS) $(BOOT_ASFLAGS) -o $@ $<

.PHONY: build run debug clean directories
