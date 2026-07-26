set shell := ["bash", "-cu"]

default:
    @just --list

build:
    pnpm --dir runtime build
    cargo build --workspace

test:
    cargo test --workspace
    pnpm --dir runtime test
    pnpm test:npm-cli

conformance:
    cargo xtask conformance
    pnpm --dir conformance/browser test

gen:
    cargo xtask gen-runtime

gen-check:
    cargo xtask gen-runtime --check

licenses:
    @test "$(cargo about --version)" = "cargo-about 0.9.1"
    cargo about generate --locked --manifest-path crates/polygl-cli/Cargo.toml about.hbs -o THIRD_PARTY_LICENSES.txt
    perl -pi -e 's/\r\n\z/\n/; s/[ \t]+\n\z/\n/' THIRD_PARTY_LICENSES.txt

licenses-check:
    @test "$(cargo about --version)" = "cargo-about 0.9.1"
    @temporary="$(mktemp)"; trap 'rm -f "$temporary"' EXIT; \
        cargo about generate --locked --manifest-path crates/polygl-cli/Cargo.toml about.hbs -o "$temporary"; \
        perl -pi -e 's/\r\n\z/\n/; s/[ \t]+\n\z/\n/' "$temporary"; \
        cmp THIRD_PARTY_LICENSES.txt "$temporary"

serve-example:
    cargo run -p polygl-cli -- build examples/triangle.rb -o dist
    python3 -m http.server 8000 --directory dist
