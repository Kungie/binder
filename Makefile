.PHONY: dev test build install release preview clean sidecar sidecar-release

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

install: build
	rm -rf /Applications/Binder.app
	cp -R $(CARGO_TARGET_DIR)/release/bundle/macos/Binder.app /Applications/

# Binder.dmg for a GitHub release: the app signed ad hoc as a whole (not notarized, so the first
# launch needs Privacy & Security > Open Anyway) beside a link to /Applications.
release: build
	rm -rf $(CARGO_TARGET_DIR)/dmg build/Binder.dmg
	mkdir -p $(CARGO_TARGET_DIR)/dmg build
	cp -R $(CARGO_TARGET_DIR)/release/bundle/macos/Binder.app $(CARGO_TARGET_DIR)/dmg/
	codesign --force --deep --sign - $(CARGO_TARGET_DIR)/dmg/Binder.app
	codesign --verify --deep --strict $(CARGO_TARGET_DIR)/dmg/Binder.app
	ln -s /Applications $(CARGO_TARGET_DIR)/dmg/Applications
	hdiutil create -quiet -volname Binder -srcfolder $(CARGO_TARGET_DIR)/dmg -format UDZO -ov build/Binder.dmg

preview:
	npm --prefix app run dev

clean:
	cargo clean
	rm -rf app/dist
