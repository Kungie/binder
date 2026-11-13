.PHONY: dev test build preview clean sidecar sidecar-release

# ~/Desktop syncs with iCloud, which stamps Finder attributes on build output and breaks codesign.
# Build outside it.
export CARGO_TARGET_DIR := $(HOME)/Library/Caches/Binder/target

# Tauri bundles binder-cli as a sidecar named after the target triple.
TRIPLE := $(shell rustc -vV | sed -n 's/^host: //p')
SIDECAR := app/src-tauri/binaries/binder-cli-$(TRIPLE)

sidecar:
	cargo build -p binder-cli
	mkdir -p app/src-tauri/binaries
	cp $(CARGO_TARGET_DIR)/debug/binder-cli $(SIDECAR)

sidecar-release:
	cargo build --release -p binder-cli
	mkdir -p app/src-tauri/binaries
	cp $(CARGO_TARGET_DIR)/release/binder-cli $(SIDECAR)

dev: sidecar
	npm --prefix app run tauri dev

test: sidecar
	cargo test --workspace
	@if [ -f app/package.json ]; then npm --prefix app test -- --run; fi

build: sidecar-release
	npm --prefix app run tauri build -- --bundles app

preview:
	npm --prefix app run dev

clean:
	cargo clean
	rm -rf app/dist
