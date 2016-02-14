KERNEL_DIR = ./kernel
LIB_DIR = ./lib
TARGET_DIR = ./target
ASM_DIR = ./asm
STAGE2_DIR = ./stage2

GEN_DIR = $(TARGET_DIR)/gen
ISO_DIR = $(TARGET_DIR)/iso

KERNEL_SOURCES = $(shell find $(KERNEL_DIR)/src -name '*.rs')
KERNEL_TARGET = $(KERNEL_DIR)/target/debug
KERNEL = $(TARGET_DIR)/kernel.elf
KERNEL_LINK = $(LIB_DIR)/link.ld

ASM_SOURCES = $(shell find $(ASM_SOURCES)/src -name '*.asm')
ASM_TARGET = $(ASM_DIR)/target

STAGE1_KERNEL = $(KERNEL_TARGET)/libkernel.a
STAGE1_ASM = $(ASM_TARGET)/long.o
STAGE1_LINK = $(LIB_DIR)/stage1.ld
STAGE1 = $(TARGET_DIR)/stage1.elf

STAGE2_TARGET = $(STAGE2_DIR)/target/release
STAGE2_SOURCES = $(shell find $(STAGE2_DIR)/src -name '*.rs')
STAGE2 = $(STAGE2_TARGET)/stage2

GEN_SOURCES = $(GEN_DIR)/page_tables.bin $(GEN_DIR)/page_tables.asm
GEN_ASM = $(TARGET_DIR)/page_tables.o
BOOT_ASM = $(ASM_TARGET)/multiboot2.o $(ASM_TARGET)/bootstrap.o $(ASM_TARGET)/long_boot.o

OBJECTS = $(BOOT_ASM) $(GEN_ASM) $(STAGE1_ASM) $(STAGE1_KERNEL)

GRUB_CFG = $(LIB_DIR)/grub.cfg
GRUB_IMAGE = $(TARGET_DIR)/image.iso

ISO_GRUB_CFG = $(ISO_DIR)/boot/grub/grub.cfg
ISO_KERNEL = $(ISO_DIR)/boot/kernel.elf

ASFLAGS = -f elf64
LDFLAGS = --gc-sections -n
GRUB_RESCUE_FLAGS = -d /usr/lib/grub/i386-pc/

CD = cd
CARGO = cargo
AS = nasm
LD = ld
MKDIR = mkdir
RM = rm
GRUB_RESCUE = grub-mkrescue
CP = cp

build: directories $(KERNEL)

image: directories $(GRUB_IMAGE)

$(ISO_GRUB_CFG): $(GRUB_CFG)
	$(CP) $< $@

$(ISO_KERNEL): $(KERNEL)
	$(CP) $< $@

$(GRUB_IMAGE): $(ISO_GRUB_CFG) $(ISO_KERNEL)
	$(GRUB_RESCUE) $(GRUB_RESCUE_FLAGS) -o $(GRUB_IMAGE) $(ISO_DIR)

$(STAGE1_KERNEL): $(KERNEL_SOURCES)
	$(CD) $(KERNEL_DIR) && $(CARGO) build

$(STAGE1_ASM) $(BOOT_ASM): $(ASM_TARGET)/%.o : $(ASM_DIR)/src/%.asm
	$(AS) $(ASFLAGS) -o $@ $<

$(GEN_ASM): $(GEN_SOURCES)
	$(AS) $(ASFLAGS) -o $@ $(GEN_DIR)/page_tables.asm

$(GEN_SOURCES): $(STAGE2)
	$(STAGE2)

$(STAGE2): $(STAGE1) $(STAGE2_SOURCES)
	$(CD) $(STAGE2_DIR) && $(CARGO) build --release

$(STAGE1): $(STAGE1_KERNEL) $(STAGE1_ASM) $(STAGE1_LINK)
	$(LD) $(LDFLAGS) -T $(STAGE1_LINK) -o $@ $(STAGE1_ASM) $(STAGE1_KERNEL)

$(TARGET_DIR) $(ASM_TARGET) $(GEN_DIR) $(ISO_DIR) $(ISO_DIR)/boot $(ISO_DIR)/boot/grub:
	$(MKDIR) -p $@

$(KERNEL): $(OBJECTS)
	$(LD) $(LDFLAGS) -T $(KERNEL_LINK) -o $@ $^

clean:
	$(RM) -rf $(TARGET_DIR) $(ASM_TARGET)
	$(CD) $(KERNEL_DIR) && $(CARGO) clean
	$(CD) $(STAGE2_DIR) && $(CARGO) clean

directories: $(TARGET_DIR) $(ASM_TARGET) $(GEN_DIR) $(ISO_DIR) $(ISO_DIR)/boot $(ISO_DIR)/boot/grub

.PHONY: build image clean directories
