# Justfile for ratchet development
# Run `just --list` to see all available commands

# Default recipe shows available commands
default:
    @just --list

# Install ratchet binary system-wide
install:
    cargo install --path .

# Build in release mode
build:
    cargo build --release

# Build in debug mode
dev:
    cargo build

# Run tests using nextest
test:
    cargo nextest run

# Run ratchet on itself
check:
    cargo run --release -- check

# Format code
fmt:
    cargo fmt

# Check formatting without modifying files
fmt-check:
    cargo fmt --check

# Run clippy linter
lint:
    cargo clippy -- -D warnings

# Clean build artifacts
clean:
    cargo clean

# Run full CI suite: format, lint, test, and check
all: fmt lint test check
    @echo "All checks passed!"
