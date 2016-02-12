TARGET_DIR = ./target
SOURCE_DIR = ./src

SOURCES = $(shell find $(SOURCE_DIR) -name '*.asm')
OBJECTS = $(SOURCES:$(SOURCE_DIR)/%.asm=$(TARGET_DIR)/%.o)

GLOBAL_SUBTARGET_DIR = $(GLOBAL_TARGET_DIR)/asm
GLOBAL_OBJECTS = $(OBJECTS:$(TARGET_DIR)/%=$(GLOBAL_SUBTARGET_DIR)/%)

AS_FLAGS = -f elf64

MKDIR = mkdir
AS = nasm
RM = rm
CP = cp

build: $(GLOBAL_SUBTARGET_DIR) $(TARGET_DIR) $(GLOBAL_OBJECTS)

clean:
	$(RM) -rf $(TARGET_DIR)

$(GLOBAL_SUBTARGET_DIR):
	$(MKDIR) -p $@

$(GLOBAL_OBJECTS): $(GLOBAL_SUBTARGET_DIR)/% : $(TARGET_DIR)/%
	$(CP) $< $@

$(TARGET_DIR):
	$(MKDIR) -p $@

$(OBJECTS): $(TARGET_DIR)/%.o : $(SOURCE_DIR)/%.asm
	$(AS) $(AS_FLAGS) -o $@ $<

.PHONY: build clean
