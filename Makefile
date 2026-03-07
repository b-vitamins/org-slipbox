EMACS ?= emacs

.PHONY: fmt
fmt:
	cargo fmt --all

.PHONY: test-rust
test-rust:
	cargo test --workspace

.PHONY: lint-rust
lint-rust:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

.PHONY: test-elisp
test-elisp:
	$(EMACS) -Q --batch -L . -l org-slipbox.el -l tests/test-org-slipbox.el -f ert-run-tests-batch-and-exit

.PHONY: test
test: test-rust test-elisp
