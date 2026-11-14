PREFIX ?= /usr/local
DESTDIR ?=

.PHONY: build check test install portable
build:
	cargo build --release --locked

portable: build
	python3 scripts/build_portable.py

check:
	cargo fmt --all --check
	cargo clippy --locked --all-targets -- -D warnings

test:
	cargo test --locked

install: portable
	python3 scripts/install.py --prefix "$(PREFIX)" --destdir "$(DESTDIR)"
