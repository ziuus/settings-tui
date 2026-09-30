PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin
DATADIR ?= $(PREFIX)/share
MANDIR ?= $(DATADIR)/man/man1
APPDIR ?= $(DATADIR)/applications

TARGET = target/release/settings-tui

.PHONY: all build test check clean install uninstall

all: build

build:
	cargo build --release

test:
	cargo test

check:
	cargo clippy --all-targets -- -D warnings
	cargo fmt --check

clean:
	cargo clean

install: build
	install -d "$(DESTDIR)$(BINDIR)"
	install -m 755 "$(TARGET)" "$(DESTDIR)$(BINDIR)/settings-tui"
	install -d "$(DESTDIR)$(APPDIR)"
	install -m 644 extra/settings-tui.desktop "$(DESTDIR)$(APPDIR)/settings-tui.desktop"
	install -d "$(DESTDIR)$(MANDIR)"
	install -m 644 extra/settings-tui.1 "$(DESTDIR)$(MANDIR)/settings-tui.1"

uninstall:
	rm -f "$(DESTDIR)$(BINDIR)/settings-tui"
	rm -f "$(DESTDIR)$(APPDIR)/settings-tui.desktop"
	rm -f "$(DESTDIR)$(MANDIR)/settings-tui.1"
