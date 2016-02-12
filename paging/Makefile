SOURCES = $(shell find src -name *.rs)
TARGET = target/debug/libpaging.rlib

build: $(TARGET)

$(TARGET): $(SOURCES)
	cargo build

clean:
	cargo clean

.PHONY: clean build
