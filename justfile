# Common dev commands for gimoji.
#
# The repo has two roots: the native workspace (`gimoji-core` + `gimoji`)
# and the standalone wasm crate (`crates/gimoji-web/`). These recipes
# hide that split so day-to-day commands don't need a `cd` or `--target`.
#
# Run `just` (no args) to see the list.

WEB := "crates/gimoji-web"

default:
    @just --list

# Build the native crates (debug).
build:
    cargo build

# Type-check the native workspace with --locked (matches CI).
check:
    cargo --locked check

# Run the native gimoji binary. Forward extra args, e.g. `just run -- --help`.
run *args:
    cargo run {{args}}

# Run the native test suite.
test:
    cargo test

# Install the native gimoji binary to ~/.cargo/bin.
install:
    cargo install --path crates/gimoji

# Format every crate (native workspace + the standalone wasm crate).
fmt:
    cargo fmt --all
    cd {{WEB}} && cargo fmt --all

# Verify formatting without rewriting (CI parity).
fmt-check:
    cargo fmt --all -- --check
    cd {{WEB}} && cargo fmt --all -- --check

# Clippy both sides with `-D warnings`.
lint:
    cargo clippy --locked --all-targets -- -D warnings
    cd {{WEB}} && cargo clippy --locked --all-targets -- -D warnings

# Build gimoji-web for wasm (debug profile).
web-build:
    cd {{WEB}} && cargo build

# Build gimoji-web with the size-optimised `[profile.web]`.
web-release:
    cd {{WEB}} && cargo build --profile web --locked

# Build, bundle, and serve gimoji-web on http://localhost:PORT (default 8000).
web-serve port='8000':
    ./scripts/serve-web.sh {{port}}
