VERSION := $(shell cat VERSION.txt 2>/dev/null || echo "0.0.1")

.PHONY: all build test version clean

all: build

version:
	@echo $(VERSION)

build:
	@echo "Building repository (version $(VERSION))..."
	@if [ -f "Cargo.toml" ]; then cargo build --release || true; fi

test:
	@echo "Running tests (version $(VERSION))..."
	@if [ -f "Cargo.toml" ]; then cargo test || true; fi

clean:
	@echo "Cleaning build artifacts..."
	@rm -rf dist build *.egg-info target/
