NIX_FILES := $(shell find . -name '*.nix' -not -path './.git/*')

.PHONY: check test fmt fmt-check lint build bless

# The whole gate, as run by hooks/pre-push and by CI.
check: fmt-check lint test

test:
	cargo test

fmt:
	nixfmt $(NIX_FILES)
	cargo fmt

fmt-check:
	nixfmt --check $(NIX_FILES)
	cargo fmt --check

lint:
	cargo clippy --all-targets -- -D warnings

build:
	cargo build

# Rewrite the golden outputs from the current formatter. Review the diff.
bless:
	STEELWOOL_BLESS=1 cargo test --test golden
