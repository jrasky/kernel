BOOT_DIRS = ./boot ./log ./log_abi ./kernel_std ./serial ./constants ./paging ./memory
KERNEL_DIRS = ./kernel ./log ./log_abi ./paging ./user ./constants ./serial ./memory ./kernel_std
STAGE2_DIRS = ./stage2 ./paging ./constants ./kernel_std ./log ./log_abi

ASM_DIR = ./asm
LIB_DIR = ./lib
TARGET_DIR = ./target

ISO_DIR = $(TARGET_DIR)/iso

KERNEL_SOURCES = $(foreach dir,$(KERNEL_DIRS),$(wildcard $(dir)/src/*) $(dir)/Cargo.toml)
BOOT_SOURCES = $(foreach dir,$(BOOT_DIRS),$(wildcard $(dir)/src/*) $(dir)/Cargo.toml)
STAGE2_SOURCES = $(foreach dir,$(STAGE2_DIRS),$(wildcard $(dir)/src/*) $(dir)/Cargo.toml)
ASM_SOURCES = $(wildcard $(ASM_DIR)/src/*)

BOOT_TARGET = ./boot/target/i686-unknown-linux-gnu/debug/libboot.a
BOOT = $(TARGET_DIR)/boot.elf
BOOT_LINK = $(LIB_DIR)/boot.ld
BOOT_ASM = $(ASM_DIR)/target/multiboot2.o $(ASM_DIR)/target/short_boot.o

KERNEL_TARGET = ./kernel/target/debug/libkernel.a
KERNEL = $(TARGET_DIR)/kernel.elf
KERNEL_MOD = $(TARGET_DIR)/kernel.mod
KERNEL_LINK = $(LIB_DIR)/link.ld
KERNEL_ASM = $(ASM_DIR)/target/util.o $(ASM_DIR)/target/long_stub.o

STAGE2 = ./stage2/target/debug/stage2

GRUB_CFG = $(LIB_DIR)/grub.cfg
GRUB_IMAGE = $(TARGET_DIR)/image.iso

ISO_GRUB_CFG = $(ISO_DIR)/boot/grub/grub.cfg
ISO_KERNEL = $(ISO_DIR)/boot/kernel.mod
ISO_BOOT = $(ISO_DIR)/boot/boot.elf

KERNEL_ASFLAGS = -f elf64
KERNEL_LDFLAGS = --gc-sections -n

BOOT_ASFLAGS = -f elf32
BOOT_LDFLAGS = --gc-sections -n -m elf_i386

GRUB_RESCUE_FLAGS = -d /usr/lib/grub/x86_64-efi/
VM_FLAGS = -enable-kvm -net none -m 1024 -drive file=/usr/share/ovmf/ovmf_x64.bin,format=raw,if=pflash,readonly -k en-us -serial stdio

CD = cd
CARGO = cargo
AS = nasm
LD = ld
MKDIR = mkdir
RM = rm
GRUB_RESCUE = grub-mkrescue
CP = cp
VM = qemu-system-x86_64

build: directories $(KERNEL)

image: directories $(GRUB_IMAGE)

run: image
	$(VM) $(VM_FLAGS) -cdrom $(GRUB_IMAGE)

clean:
	$(RM) -rf $(TARGET_DIR) $(ASM_DIR)/target
	$(CD) ./stage2 && cargo clean
	$(CD) ./kernel && cargo clean
	$(CD) ./boot && cargo clean

$(ISO_GRUB_CFG): $(GRUB_CFG)
	$(CP) $< $@

$(ISO_KERNEL): $(KERNEL_MOD)
	$(CP) $< $@

$(GRUB_IMAGE): $(ISO_GRUB_CFG) $(ISO_KERNEL)
	$(GRUB_RESCUE) $(GRUB_RESCUE_FLAGS) -o $(GRUB_IMAGE) $(ISO_DIR)

$(BOOT_TARGET): $(BOOT_SOURCES)
	$(CD) ./boot && $(CARGO) build

$(KERNEL_TARGET): $(KERNEL_SOURCES)
	$(CD) ./kernel && $(CARGO) build

$(KERNEL_ASM): $(ASM_DIR)/target/%.o : $(ASM_DIR)/src/%.asm
	$(AS) $(KERNEL_ASFLAGS) -o $@ $<

$(BOOT_ASM): $(ASM_DIR)/target/%.o : $(ASM_DIR)/src/%.asm
	$(AS) $(BOOT_ASFLAGS) -o $@ $<

$(STAGE2): $(STAGE2_SOURCES)
	$(CD) ./stage2 && $(CARGO) build

$(BOOT): $(BOOT_TARGET) $(BOOT_ASM) $(BOOT_LINK)
	$(LD) $(BOOT_LDFLAGS) -T $(BOOT_LINK) -o $@ $(BOOT_ASM) $(BOOT_TARGET)

$(KERNEL): $(KERNEL_TARGET) $(KERNEL_ASM) $(KERNEL_LINK)
	$(LD) $(KERNEL_LDFLAGS) -T $(KERNEL_LINK) -o $@ $(KERNEL_ASM) $(KERNEL_TARGET)

$(KERNEL_MOD): $(KERNEL) $(STAGE2)
	$(STAGE2)

$(TARGET_DIR) $(ASM_DIR)/target $(ISO_DIR) $(ISO_DIR)/boot $(ISO_DIR)/boot/grub:
	$(MKDIR) -p $@

directories: $(TARGET_DIR) $(ASM_DIR)/target $(ISO_DIR) $(ISO_DIR)/boot $(ISO_DIR)/boot/grub

.PHONY: build image run clean directories
