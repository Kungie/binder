.PHONY: dev test build install release release-check preview clean sidecar sidecar-release vpn vpn-release

# ~/Desktop syncs with iCloud, which stamps Finder attributes on build output and breaks codesign.
# Build outside it.
export CARGO_TARGET_DIR := $(HOME)/Library/Caches/Binder/target

# Tauri bundles binder-cli as a sidecar named after the target triple.
TRIPLE := $(shell rustc -vV | sed -n 's/^host: //p')
SIDECAR := app/src-tauri/binaries/binder-cli-$(TRIPLE)

# The private key that signs updates lives only in the Keychain; the app holds the public key.
UPDATER_KEY := -s io.github.kungie.binder.updater -a release
# The app is signed with this self-signed identity from the login Keychain, so its code identity stays the same
# from one build to the next and macOS keeps its permissions (Local Network, Finder control) across updates.
# Without it a build is signed ad hoc, and macOS sees every update as a new app.
SIGN_ID := Binder Self-Signed
SIGN = $(shell security find-certificate -c "$(SIGN_ID)" >/dev/null 2>&1 && echo "$(SIGN_ID)" || echo -)
VERSION = $(shell node -p "require('./app/package.json').version")

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
	python3 -m unittest discover -s vpn/server

build: sidecar-release
	npm --prefix app run tauri build -- --bundles app

install: build
	codesign --force --deep --sign "$(SIGN)" $(CARGO_TARGET_DIR)/release/bundle/macos/Binder.app
	rm -rf /Applications/Binder.app
	cp -R $(CARGO_TARGET_DIR)/release/bundle/macos/Binder.app /Applications/

# A release needs its notes and the update key before anything is built.
release-check:
	@test -f build/notes.md || { echo "Write build/notes.md first."; exit 1; }
	@security find-generic-password $(UPDATER_KEY) >/dev/null 2>&1 || { echo "No update signing key in the Keychain."; exit 1; }
	@security find-certificate -c "$(SIGN_ID)" >/dev/null 2>&1 || { echo "No $(SIGN_ID) identity in the Keychain."; exit 1; }

# Binder.dmg for a GitHub release: the app signed with $(SIGN_ID) as a whole (not notarized, so the first
# launch needs Privacy & Security > Open Anyway) beside a link to /Applications, in a window laid out
# by scripts/dmg.sh over app/dmg/background.tiff (drawn from background.svg by scripts/dmg-background.swift).
# Next to it, the update Binder installs from Settings: the same app as a signed tarball, and latest.json.
release: release-check build
	rm -rf $(CARGO_TARGET_DIR)/dmg build/Binder.dmg build/Binder.app.tar.gz build/Binder.app.tar.gz.sig build/latest.json
	mkdir -p $(CARGO_TARGET_DIR)/dmg build
	cp -R $(CARGO_TARGET_DIR)/release/bundle/macos/Binder.app $(CARGO_TARGET_DIR)/dmg/
	codesign --force --deep --sign "$(SIGN_ID)" $(CARGO_TARGET_DIR)/dmg/Binder.app
	codesign --verify --deep --strict $(CARGO_TARGET_DIR)/dmg/Binder.app
	ln -s /Applications $(CARGO_TARGET_DIR)/dmg/Applications
	scripts/dmg.sh $(CARGO_TARGET_DIR)/dmg app/dmg/background.tiff build/Binder.dmg
	COPYFILE_DISABLE=1 tar -czf build/Binder.app.tar.gz -C $(CARGO_TARGET_DIR)/dmg Binder.app
	@echo "Signing build/Binder.app.tar.gz"
	@TAURI_SIGNING_PRIVATE_KEY="$$(security find-generic-password $(UPDATER_KEY) -w)" TAURI_SIGNING_PRIVATE_KEY_PASSWORD="" \
		npm --prefix app exec -- tauri signer sign "$(CURDIR)/build/Binder.app.tar.gz" >/dev/null
	node app/scripts/latest-json.ts $(VERSION) build/notes.md build/Binder.app.tar.gz.sig > build/latest.json
	$(MAKE) vpn-release

# Binder VPN, the friends' menu bar app (vpn/): its own bundle, signed the same way, in a plain DMG.
vpn:
	cd vpn && ../app/node_modules/.bin/tauri build --bundles app

vpn-release: vpn
	rm -rf $(CARGO_TARGET_DIR)/vpn-dmg build/Binder-VPN.dmg
	mkdir -p $(CARGO_TARGET_DIR)/vpn-dmg build
	cp -R "$(CARGO_TARGET_DIR)/release/bundle/macos/Binder VPN.app" $(CARGO_TARGET_DIR)/vpn-dmg/
	codesign --force --deep --sign "$(SIGN)" "$(CARGO_TARGET_DIR)/vpn-dmg/Binder VPN.app"
	ln -s /Applications $(CARGO_TARGET_DIR)/vpn-dmg/Applications
	hdiutil create -volname "Binder VPN" -srcfolder $(CARGO_TARGET_DIR)/vpn-dmg -ov -format UDZO build/Binder-VPN.dmg

preview:
	npm --prefix app run dev

clean:
	cargo clean
	rm -rf app/dist
