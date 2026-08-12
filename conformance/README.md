# Conformance tests

PolyGL conformance is split into the three layers defined by the design:

- `l1-render`: behavioral equivalence through deterministic rendering.
- `l2-hir-snapshots`: language-specific HIR regression snapshots.
- `l3-neutral-hir`: normalized HIR equality for the neutral subset.

`cases.json` is the shared declarative inventory for the Rust and browser
runners. `polygl-conformance` implements capability-based selection,
renderer-keyed L1 frame comparison, language-specific L2 snapshot verification,
normalized L3 equality, and GPU split/backend checks. `cargo xtask conformance`
checks six shared L1 baselines, 22 L2 snapshots, two cross-language neutral L3
comparisons and snapshots, one positive GLSL case, and two expected GPU
diagnostics. It also rejects an advertised feature without a case. The
`lit-cubes` L1 case exercises the Tier 2 camera, light, retained mesh, node
transform, and material path.

`adapter-corpus.json` separately declares parser-recovery, directive
attachment, and Unicode-identifier regressions for every bundled adapter. Its
runner validates all primary, label, and suggestion spans against the original
source, requires multiple E0100 diagnostics for independent syntax failures,
and confirms that NFC and NFD source identifiers remain distinct.

`semantic-cases.json` is the executable Host semantics inventory. Each fixture
is compiled in Rust, imported by a fresh Node process, and run against a small
observable implementation of the canonical runtime operations. Exact event or
error expectations cover i32 wrapping, floating-point special values, source
division and remainder rules, left-to-right and short-circuit evaluation,
array aliasing and bounds, map keys, structured loops, and debug checks. This
layer intentionally does not reuse a source-language interpreter as its oracle:
Common Core HIR semantics are normative after adapter lowering.

`pnpm --dir conformance/browser test` rebuilds every L1 case in Ruby, PHP, and Perl
with the CLI and compares both real WebGL2 framebuffers against the same bytes
under pinned Chromium + SwiftShader. It
also starts the plasma case to exercise driver compilation, linking, automatic
uniform reflection, and compile-time material name resolution.

Firefox and WebKit use a separate portability smoke test because their
headless software renderers do not share Chromium/SwiftShader's renderer key or
byte-exact baseline. Install them with `pnpm --dir conformance/browser exec
playwright install firefox webkit`, then run `pnpm --dir conformance/browser
test:portability`. This is suitable for a non-blocking scheduled/manual job;
the Chromium/SwiftShader suite remains the deterministic required gate.
