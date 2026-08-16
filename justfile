set dotenv-load

import ".shared/common.just"

default:
    @just --list

# Build a debug binary.
build:
    cargo build

# Build an optimized binary.
build-release:
    cargo build --release

# Build the reproducible Nix package.
nix-build:
    nix build .#qownnotes-tui

# Run the application directly through Nix with the "test" session; pass CLI arguments after `--`.
nix-run *args:
    nix run .#qownnotes-tui -- --session test {{ args }}

# Build the Nix package, then run the resulting binary with the "test" session.
nix-build-run *args: nix-build
    ./result/bin/qownnotes-tui --session test {{ args }}

# Run the application; pass CLI arguments after `--`.
run *args:
    cargo run -- {{ args }}

# Run all tests.
test:
    cargo test --all-targets

# Check formatting without changing files.
fmt-check:
    cargo fmt --check

# Run Clippy with warnings denied.
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Check formatting, lint, and tests.
check: fmt-check lint test

# Check Rust dependencies for known advisories.
audit:
    nix run nixpkgs#cargo-audit -- audit

# Validate all flake outputs.
flake-check:
    nix flake check
