SOURCES = $(shell find src -name '*.rs')
TARGET = target/release/stage2

GLOBAL_TARGET = $(GLOBAL_TARGET_DIR)/stage2

CP = cp

build: $(GLOBAL_TARGET)

$(GLOBAL_TARGET): $(TARGET)
	$(CP) $(TARGET) $(GLOBAL_TARGET)

$(TARGET): $(SOURCES)
	cargo build --release

clean:
	cargo clean

.PHONY: clean build
