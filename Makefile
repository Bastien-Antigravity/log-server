VERSION := $(strip $(shell cat VERSION.txt 2>/dev/null))
$(if $(VERSION),,$(error VERSION.txt is missing or empty))

.PHONY: all build test version clean

all: build

version:
	@echo $(VERSION)

build:
	@echo "Building repository (version $(VERSION))..."
	@if [ -f "Cargo.toml" ]; then cargo build --release; fi

test:
	@echo "Running tests (version $(VERSION))..."
	@if [ -f "Cargo.toml" ]; then cargo test; fi

clean:
	@echo "Cleaning build artifacts..."
	@rm -rf dist build *.egg-info target/
