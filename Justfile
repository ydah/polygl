set shell := ["bash", "-cu"]

default:
    @just --list

build:
    pnpm --dir runtime build
    cargo build --workspace

test:
    cargo test --workspace
    pnpm --dir runtime test

conformance:
    cargo xtask conformance
    pnpm --dir conformance/browser test

gen:
    cargo xtask gen-runtime

gen-check:
    cargo xtask gen-runtime --check

serve-example:
    cargo run -p polygl-cli -- build examples/triangle.rb -o dist
    python3 -m http.server 8000 --directory dist
