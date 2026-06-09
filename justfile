# patina developer tasks — `just` command runner

_default:
    @just --list

# One-time: point git at the committed hooks (run after cloning)
setup:
    git config core.hooksPath .githooks
    @echo "→ core.hooksPath set to .githooks (pre-commit will auto-fmt)"

# Build the workspace
build:
    cargo build

# Build release with LTO
build-release:
    cargo build --release

# Run all tests
test:
    cargo test

# Run tests with release optimizations (faster)
test-release:
    cargo test --release

# Run clippy with workspace lints
lint:
    cargo clippy --workspace --all-targets

# Format all code
fmt:
    cargo fmt --all

# Check formatting without changing files
fmt-check:
    cargo fmt --all --check

# Build documentation
docs:
    cargo doc --no-deps --document-private-items

# Run the patina server
run:
    cargo run -p patina

# Run all checks: fmt, clippy, test, doc
check-all: fmt-check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test
    cargo doc --no-deps --document-private-items

# Clean build artifacts
clean:
    cargo clean

# Update dependencies
update:
    cargo update
