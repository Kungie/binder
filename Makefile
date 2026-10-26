.PHONY: build test cli clean scratch

# ~/Desktop syncs with iCloud, which stamps Finder attributes on build products and breaks codesign.
# Keep .build as a link to a folder outside iCloud.
SCRATCH := $(HOME)/Library/Caches/Binder/build

# With only the Command Line Tools, Swift Testing's macro plugin doesn't load through the
# plugin server. Loading the macro library in-process works.
TESTING_MACROS := $(shell xcode-select -p)/usr/lib/swift/host/plugins/testing/libTestingMacros.dylib
TEST_FLAGS := $(if $(wildcard $(TESTING_MACROS)),-Xswiftc -load-plugin-library -Xswiftc $(TESTING_MACROS),)

scratch:
	@if [ ! -L .build ]; then rm -rf .build; mkdir -p "$(SCRATCH)"; ln -s "$(SCRATCH)" .build; fi


build: scratch
	swift build

test: scratch
	swift test $(TEST_FLAGS) $(if $(FILTER),--filter $(FILTER),)

cli: scratch
	swift build -c release --product binder
	@echo "Built $$(swift build -c release --show-bin-path)/binder"

clean:
	rm -rf "$(SCRATCH)" build
