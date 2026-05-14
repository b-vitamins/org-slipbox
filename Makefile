EMACS ?= emacs
BUILD_FEATURES ?=
SLIPBOX_CC ?= $(shell command -v cc 2>/dev/null || command -v clang 2>/dev/null || command -v gcc 2>/dev/null)
SLIPBOX_CARGO_ENV = $(if $(SLIPBOX_CC),CC="$(SLIPBOX_CC)")
GUIX ?= guix
GUIX_MANIFEST ?= manifest.scm
GUIX_SHELL = $(GUIX) shell -m $(GUIX_MANIFEST) --

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
	$(EMACS) -Q --batch --eval '(setq load-prefer-newer t)' -L . -l org-slipbox.el -l tests/test-org-slipbox.el -f ert-run-tests-batch-and-exit

.PHONY: test
test: test-rust test-elisp check-release-metadata

PROFILE ?= ci

.PHONY: bench
bench:
	$(SLIPBOX_CARGO_ENV) cargo run --bin slipbox-bench -- run --profile $(PROFILE)

.PHONY: bench-check
bench-check:
	$(SLIPBOX_CARGO_ENV) cargo run --bin slipbox-bench -- check --profile $(PROFILE)

.PHONY: guix-shell
guix-shell:
	$(GUIX) shell -m $(GUIX_MANIFEST)

.PHONY: guix-build
guix-build:
	$(GUIX_SHELL) $(MAKE) build

.PHONY: guix-build-system-sqlite
guix-build-system-sqlite:
	$(GUIX_SHELL) $(MAKE) build-system-sqlite

.PHONY: guix-test
guix-test:
	$(GUIX_SHELL) $(MAKE) test

.PHONY: guix-lint-rust
guix-lint-rust:
	$(GUIX_SHELL) $(MAKE) lint-rust

.PHONY: guix-bench-check
guix-bench-check:
	$(GUIX_SHELL) $(MAKE) bench-check

.RECIPEPREFIX := >

.PHONY: check-release-metadata
check-release-metadata:
>@version="$$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)"; \
>if [ -z "$$version" ]; then \
>	echo "Cargo.toml: could not determine workspace package version"; \
>	exit 1; \
>fi; \
>status=0; \
>if ! grep -Fq "The latest shipped release is \`$$version\`" README.md; then \
>	echo "README.md: latest shipped release line does not mention $$version"; \
>	status=1; \
>fi; \
>for file in $$(find . -name '*.el' -not -path './.git/*' | sort); do \
>	if ! grep -Fxq ";; Copyright (C) 2026 Ayan Das" "$$file"; then \
>		echo "$$file: missing expected copyright header"; \
>		status=1; \
>	fi; \
>	case "$$file" in \
>		./tests/*) ;; \
>		*) \
>			if grep -q '^;; Version:' "$$file"; then \
>				if ! grep -Fxq ";; Version: $$version" "$$file"; then \
>					echo "$$file: Version header does not match $$version"; \
>					status=1; \
>				fi; \
>			else \
>				echo "$$file: missing Version header"; \
>				status=1; \
>			fi; \
>			;; \
>	esac; \
>done; \
>exit $$status
