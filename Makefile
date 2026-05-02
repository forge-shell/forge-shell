.PHONY: check check-windows check-windows-clippy check-windows-tests test fmt

# Default: run all local checks
check: fmt test check-windows-clippy

fmt:
	cargo fmt --all -- --check

test:
	cargo test --workspace

# Lint for Windows without Docker — catches cfg-gated unused params,
# platform-specific lint variants, and Windows-only compile errors.
# Requires: rustup target add x86_64-pc-windows-gnu && brew install mingw-w64
check-windows-clippy:
	cargo clippy --target x86_64-pc-windows-gnu --workspace --all-targets -- -D warnings

# Run tests under Wine via cross (requires Docker + cargo install cross).
check-windows-tests:
	cross test --target x86_64-pc-windows-gnu --workspace

# Both Windows checks in one shot
check-windows: check-windows-clippy check-windows-tests
