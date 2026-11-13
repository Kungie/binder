.PHONY: dev test build preview clean

# ~/Desktop syncs with iCloud, which stamps Finder attributes on build output and breaks codesign.
# Build outside it.
export CARGO_TARGET_DIR := $(HOME)/Library/Caches/Binder/target

dev:
	npm --prefix app run tauri dev

test:
	cargo test --workspace
	@if [ -f app/package.json ]; then npm --prefix app test -- --run; fi

build:
	npm --prefix app run tauri build -- --bundles app

preview:
	npm --prefix app run dev

clean:
	cargo clean
	rm -rf app/dist
