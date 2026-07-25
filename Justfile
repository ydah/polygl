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

gen:
    cargo xtask gen-runtime

gen-check:
    cargo xtask gen-runtime --check

serve-example:
    @echo "serve-example will become runnable with the first end-to-end example in M1." >&2
    @exit 1
