SOURCES = $(shell find src -name *.rs)
TARGET = target/debug/libkernel.a

GLOBAL_TARGET = $(GLOBAL_TARGET_DIR)/libkernel.a

CP = cp

build: $(GLOBAL_TARGET)

$(GLOBAL_TARGET): $(TARGET)
	$(CP) $(TARGET) $(GLOBAL_TARGET)

$(TARGET): $(SOURCES)
	cargo build

clean:
	cargo clean

.PHONY: clean build
