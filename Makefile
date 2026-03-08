EMACS ?= emacs
BUILD_FEATURES ?=
SLIPBOX_CC ?= $(shell command -v cc 2>/dev/null || command -v clang 2>/dev/null || command -v gcc 2>/dev/null)
SLIPBOX_CARGO_ENV = $(if $(SLIPBOX_CC),CC="$(SLIPBOX_CC)")

.PHONY: build
build:
	$(SLIPBOX_CARGO_ENV) cargo build --release --locked $(BUILD_FEATURES)

.PHONY: build-system-sqlite
build-system-sqlite:
	cargo build --release --locked --no-default-features --features system-sqlite

.PHONY: install-daemon
install-daemon:
	$(SLIPBOX_CARGO_ENV) cargo install --path . --locked

.PHONY: install-daemon-system-sqlite
install-daemon-system-sqlite:
	cargo install --path . --locked --no-default-features --features system-sqlite

.PHONY: fmt
fmt:
	cargo fmt --all

.PHONY: test-rust
test-rust:
	$(SLIPBOX_CARGO_ENV) cargo test --workspace

.PHONY: lint-rust
lint-rust:
	$(SLIPBOX_CARGO_ENV) cargo clippy --workspace --all-targets --all-features -- -D warnings

.PHONY: test-elisp
test-elisp:
	$(EMACS) -Q --batch -L . -l org-slipbox.el -l tests/test-org-slipbox.el -f ert-run-tests-batch-and-exit

.PHONY: test
test: test-rust test-elisp

PROFILE ?= ci

.PHONY: bench
bench:
	$(SLIPBOX_CARGO_ENV) cargo run --bin slipbox-bench -- run --profile $(PROFILE)

.PHONY: bench-check
bench-check:
	$(SLIPBOX_CARGO_ENV) cargo run --bin slipbox-bench -- check --profile $(PROFILE)
