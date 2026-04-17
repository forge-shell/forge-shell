# Contributing to Forge Shell

## Running checks locally

Before pushing, run the same checks CI will run:

```bash
cargo fmt --all -- --check   # check formatting
cargo clippy --workspace --all-targets -- -D warnings  # lint
cargo test --workspace       # tests
```

To auto-fix formatting:

```bash
cargo fmt --all
```

## Branch policy

- `main` is protected — all three CI platforms must be green before merging
- Windows failures are hard blockers, not optional

## Code style

- Edition: Rust 2024
- Max line width: 100 characters
- Imports grouped: std → external crates → internal crates