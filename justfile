# Task runner for kitty-markdown.nvim. The recipes are the commit gates
# described in AGENTS.md: run lint, test, and build before every commit.

helper := "helper/Cargo.toml"

# Cargo features for the mermaid phases whose tests must run, for example
# "--features phase2". Empty while only phase 1 is delivered.
phases := ""

default: lint test

lint:
    stylua --check lua plugin tests
    selene lua plugin tests
    cargo fmt --manifest-path {{helper}} -- --check
    cargo clippy --manifest-path {{helper}} --all-targets {{phases}} -- -D warnings
    markdownlint-cli2 "**/*.md"

test:
    nvim --clean -l tests/run.lua
    cargo test --manifest-path {{helper}} {{phases}}

build:
    cargo build --release --manifest-path {{helper}}

# Render oracle PNGs for every helper/tests/goldens/<type>/<case>.mmd with the
# pinned mermaid-cli, at the pixel size the helper reports for each case.
goldens:
    npm ci --prefix helper/oracle
    node helper/oracle/goldens.mjs helper/tests/goldens

# Regenerate the LALR tables for the vendored jison grammars.
jison-tables:
    node helper/grammars/build-tables.mjs
