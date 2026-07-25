set shell := ["bash", "-cu"]

default:
    @just --list

build:
    cargo build --workspace
    pnpm --dir runtime build

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
