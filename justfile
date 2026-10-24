# SlintLu task runner — run `just` to list recipes.

set shell := ["bash", "-euo", "pipefail", "-c"]

renderer := env("SLINTLU_RENDERER", "auto")

# List available recipes
default:
    @just --list --unsorted

# Build everything (debug)
build *args:
    cargo build --all-targets {{ args }}

# Run a shell configuration (file or directory) with the chosen renderer
run config="examples/starter" renderer=renderer *args:
    cargo run -- --config {{ config }} --renderer {{ renderer }} {{ args }}

# Compile-check a shell configuration without opening windows
check config="examples/starter":
    cargo run -- check {{ config }}

# Run one of the Rust examples, e.g. `just example hello_bar vulkan`
example name="hello_bar" renderer=renderer:
    SLINTLU_RENDERER={{ renderer }} RUST_LOG=${RUST_LOG:-info} cargo run --example {{ name }}

# Format all sources
fmt:
    cargo fmt --all

# Verify formatting (CI)
fmt-check:
    cargo fmt --all -- --check

# Clippy with warnings denied, for every feature combination CI uses
lint:
    cargo clippy --all-targets -- -D warnings
    cargo clippy --all-targets --no-default-features -- -D warnings

# Unit and integration tests that need no compositor
test *args:
    cargo test --all-targets {{ args }}

# Build API docs with warnings denied
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items

# Licences, advisories, duplicate and banned crates (needs cargo-deny)
deny:
    cargo deny check

# Spelling (needs typos-cli)
typos:
    typos

# Render the starter shell inside a headless sway and check the screenshot (needs sway + grim)
headless renderer="software":
    cargo build
    scripts/headless-test.sh {{ renderer }}

# Everything CI runs, in the same order
ci: fmt-check lint test doc

# Remove build artefacts
clean:
    cargo clean
