---
title: Improvement proposal resolution
description: Adversarially reviewed disposition of every 2026 improvement proposal.
---

# Improvement proposal resolution

This document records the disposition of every numbered proposal in
`.idea/impv.md` against the implementation completed on 2026-08-12. A passing
test is not treated as proof by itself: each change was also checked against
malformed input, cross-layer contracts, deterministic output, cleanup after
failure, and the public API boundary.

The decisions mean:

- **complete**: the behavior and an adversarial or conformance check exist;
- **boundary complete**: the useful, sound part is implemented, while a named
  platform or architecture boundary prevents the broader wording;
- **deferred**: implementing it now would create an unsound contract or
  infrastructure without a real consumer. The row gives a re-entry condition.

“Deferred” is an explicit engineering decision, not an assertion that the
requested feature was implemented.

## Artifact, server, value, and cross-layer contracts (1–24)

| # | Decision | Resolution |
| ---: | --- | --- |
| 1 | boundary complete | A complete adjacent staging generation replaces the old directory and restores it on an activation error. Portable filesystems cannot replace a non-empty directory with one rename, so the documented two-rename path has a brief absence/crash window. Use versioned directories plus a server-owned pointer when uninterrupted publication is required. |
| 2 | complete | Publishing a complete generation removes every stale file; manifest-diff deletion is unnecessary and less safe. |
| 3 | complete | A symlink output root is rejected, all children are created in a fresh staging root, and output/input containment is rejected before publication. |
| 4 | complete | `none`, `external`, and `inline` Source Map modes are supported in the compiler and CLI. |
| 5 | complete | `sourcesContent` is an independent opt-in and is invalid when maps are disabled. |
| 6 | complete | Source names use portable `/` paths without leaking an absolute user path; out-of-project inputs fall back to a basename. |
| 7 | complete | Asset realpaths must remain below the canonical source directory; escaping symlinks have regression coverage. |
| 8 | complete | A component trie rejects file/directory prefix conflicts before any output is activated. |
| 9 | complete | Paths use Unicode Default Caseless Matching (Unicode 16), canonical decomposition, case folding, Windows device names, forbidden characters, and trailing-dot/space checks. Tests include sharp-s, sigma, and ypogegrammeni cases. |
| 10 | complete | HTTP paths are decoded exactly once, invalid escapes are rejected, and traversal checks run after decoding. |
| 11 | complete | Successful compilation returns the source and resolved asset watch set; source/asset changes rebuild without swallowing edits made during compilation. |
| 12 | complete | Asset collection runs on reachable Host LIR after domain splitting and effect-aware DCE. |
| 13 | complete | The request byte limit is checked immediately after each read, before accepting a header terminator. |
| 14 | complete | Static HTTP and WebSocket requests share loopback `Host`/origin validation. |
| 15 | complete | HTTP connections have bounded concurrency, request bytes, read time, and write time. |
| 16 | complete | Unsupported FIN/RSV/opcode/masking/fragmentation forms are rejected as protocol errors. |
| 17 | complete | Broadcasts pass through one ordered worker queue instead of spawning a thread per event. |
| 18 | complete | Maps are null-prototype records and access goes through own-property runtime helpers. |
| 19 | complete | Struct construction uses safe own data properties, including `__proto__`. |
| 20 | complete | Common Core defines missing Map keys and the located `mapGet` helper enforces the behavior without prototype lookup. |
| 21 | complete | Generated app and shader artifacts carry separate ABI markers; runtime startup rejects missing/mismatched markers before drawing. The provenance manifest also records compiler, runtime/shader ABI, adapter API, and HIR/LIR/builtin schemas. The browser deliberately does not trust the deployment manifest as runtime input. |
| 22 | complete | Every smoke/conformance execution path selects declarative cases from adapter capabilities and `required_features`. |
| 23 | complete | `DiagnosticCode` metadata is normative and `docs/errors.md` is generated; severity, producer, and required-fix rules are validated at external compiler boundaries. |
| 24 | complete | NaN, infinities, negative zero, saturation, and i32 limits are specified and tested for Host conversion helpers; unsafe GPU integer conversion is diagnosed. |

## Compiler and IR (25–49)

| # | Decision | Resolution |
| ---: | --- | --- |
| 25 | complete | `polygl-core::Compiler` now owns registry, analysis, compile orchestration, and structured outputs. |
| 26 | complete | The same compiler facade used by the CLI is public; the CLI retains only I/O and packaging policy. |
| 27 | complete | HIR, validated typed HIR, domain-resolved LIR, and split program stages have distinct wrappers. |
| 28 | complete | A central stage validator runs at transitions and reports the stage/pass that violated an invariant. |
| 29 | complete | Ordered pass descriptors record validation, elapsed time, and stage statistics. |
| 30 | boundary complete | Dependency and split graphs use stable `SymbolId` values. Replacing every public IR name is reserved for schema v2, which must define non-persistent serialized IDs, an accompanying name table, and backend snapshot equivalence. |
| 31 | complete | Domain propagation uses an iterative dependency graph and deterministic SCC analysis. |
| 32 | complete | Host-to-GPU errors include the deterministic shortest dependency path. |
| 33 | complete | LIR expressions explicitly classify purity/effects. |
| 34 | complete | Reachability DCE removes unused Host definitions while retaining effectful initializers and required entry points. |
| 35 | deferred | SCCP needs a CFG/SSA lattice that the structured-tree LIR does not have. Re-enter after unreachable branches are a measured size/correctness problem and a CFG, phi-equivalent semantics, source-span model, and semantic differential oracle exist. |
| 36 | deferred | Inlining cannot currently preserve call-site source ownership and is not needed to emit GPU functions. Re-enter with corpus evidence, non-recursive SCC checks, cost/node budgets, effect/evaluation-order preservation, and an inline-frame Source Map model. |
| 37 | complete | Specialization is bounded by central compile budgets and the diagnostic reports the specialization chain/details. |
| 38 | complete | The post-DCE reachable program is the sole source of packaged asset references. |
| 39 | complete | Diagnostics, pending specializations, SCCs, dependency paths, manifests, and external artifacts have stable ordering. |
| 40 | deferred | Human dumps and schema constants exist, but a versioned machine JSON format has no consumer or migration policy. Re-enter with one real consumer, a serde model separate from internal enums, golden fixtures, and a compatibility policy. |
| 41 | deferred | A persistent cache would add invalidation, corruption, eviction, and source-confidentiality risks without measured compile pressure. Re-enter when benchmarks justify it and the key includes compiler, adapter, builtin/runtime ABI, options, and source hashes with corruption fallback and reproducibility tests. |
| 42 | deferred | Compilation has one `SourceFile`, no import graph, and therefore no sound dependency invalidation unit. Re-enter after module identity/import semantics exist with edit/delete/rename/cycle invalidation tests. |
| 43 | deferred | Spawning work cannot cancel synchronous adapter/type/backend code. Re-enter when a cancellation token reaches every long pass/walker and rapid-edit tests prove old generations never publish. |
| 44 | complete | Source bytes, nodes, nesting, functions, shaders, specializations, and diagnostics use central budgets. |
| 45 | boundary complete | Deep dependency walking is iterative and typed stages reject nesting before recursive lowering. Full closure requires adversarial deep-nesting corpora for each upstream parser, followed by parser-specific iterative lowering only where those tests expose a stack risk. |
| 46 | boundary complete | Dependency analysis was extracted to a focused iterative module and stage/metrics/effect responsibilities were separated. Mechanical file splitting is not accepted solely to reduce line count; future splits require a stable responsibility boundary and tests. |
| 47 | complete | Publicly reachable malformed split and JavaScript-backend inputs return structured errors instead of panicking. Private constructors may retain checked internal invariants; any future public deserializer must enter through the stage validator. |
| 48 | complete | Adapter, type, split, optimizer, JavaScript, GLSL, and output byte metrics are available through `--profile`. |
| 49 | complete | The reproducible manifest records source and artifact hashes, compiler/adapter/schema/ABI/options provenance, and intentionally omits time. |

## Adapters, diagnostics, and conformance (50–101)

| # | Decision | Resolution |
| ---: | --- | --- |
| 50 | complete | `ADAPTER_API_VERSION` identifies the object-safe lowering contract and is recorded in artifact provenance; any future dynamic plugin must compare it before invocation. |
| 51 | complete | Tier 2 is split into meshes, scene nodes, cameras, and textures in addition to the existing core, collection, class, shader, and language-sugar tags. |
| 52 | complete | Every external feature spelling includes a semantic version suffix such as `maps-v1`; unknown versions are rejected rather than silently downgraded. |
| 53 | complete | `polygl-core::Compiler` owns the ordered built-in registry, and CLI plus conformance resolve adapters through it instead of maintaining independent matches. |
| 54 | complete | `cargo xtask capabilities` generates `docs/capabilities.md` from adapter declarations. |
| 55 | complete | The capability generator rejects an advertised feature without a manifest case for that language and feature. |
| 56 | complete | `new-adapter` creates a compilable crate, API metadata, positioned diagnostic test, capability stub, language mapping document, conformance checklist, and example stub. |
| 57 | complete | Diagnostic validation uses registry metadata to require a concrete rewrite suggestion on fixable adapter E02xx rejections; adapter corpora verify the rule. |
| 58 | complete | Public diagnostics use the closed `DiagnosticCode` type; parsing arbitrary strings happens only at CLI/manifest boundaries. |
| 59 | complete | Code, severity, title, explanation, producer, fixability, and introduction version have one typed registry that generates the error reference. |
| 60 | complete | Diagnostics carry multiple suggestions and distinguish machine-applicable, maybe-incorrect, and placeholder edits. |
| 61 | complete | Human, JSON, SARIF, and LSP output are adapters over structured ranges, labels, notes, and fixes. |
| 62 | complete | Exact/family warning selectors, allow/deny precedence, deny-all, and a maximum warning count are supported and tested. |
| 63 | boundary complete | Locked parser versions plus a shared adversarial corpus continuously check the spans, recovery, comments, and identifiers PolyGL relies on. Importing entire upstream project corpora is deferred until each corpus has pinned provenance/license, an update policy, and a triage owner; an unversioned network download would make CI less trustworthy. |
| 64 | complete | Ruby, PHP, and Perl have isolated arbitrary-byte fuzz targets that require valid UTF-8 source boundaries and never trust parser spans; the valid full-pipeline target reaches both backends. |
| 65 | complete | Each parser recovers at least two independent syntax diagnostics in a single source, with valid primary/related/fix spans. |
| 66 | boundary complete | Shared and adapter-specific tests fix attached, detached, malformed, inline, cross-function, ordinary/doc-comment, and multiple-error behavior. Add Unicode line-separator variants only if an upstream parser begins accepting them as source newlines; treating unsupported separators as comment adjacency would contradict that parser's grammar. |
| 67 | complete | Source identifiers remain byte-spelled, case-sensitive, and NFC/NFD-distinct; format controls/combining ambiguity are rejected in portable annotations, and target identifiers encode original UTF-8 bytes hygienically. |
| 68 | deferred | The Perl lowerer is one stateful tree-sitter visitor whose scope, annotations, expression context, and diagnostics share traversal state. Splitting it by line count alone would increase hidden coupling. Re-enter when change history exposes an independently testable responsibility or when a second maintainer needs module ownership; preserve one traversal state and snapshot equivalence. |
| 69 | boundary complete | Every render/HIR, host-semantic, and parser-adversarial case is declarative and selected from a validated manifest. Separate suite manifests are intentional because framebuffer baselines, execution events, and parser diagnostics have different schemas; there is no hard-coded runner inventory. |
| 70 | complete | A fresh-process Node runner executes generated JavaScript against an observable mock runtime and compares values, call order, and located failures. |
| 71 | complete | Semantic and backend tests cover i32 endpoints, wrapping addition/subtraction, unary negation, `Math.imul`, and the checked division edge. |
| 72 | boundary complete | Host tests retain negative zero, NaN, infinities, and conversion behavior. Exact GPU NaN/subnormal payloads are deliberately not promised by WebGL/GLSL; unsafe integer conversions are diagnosed and finite shader arithmetic is covered instead. |
| 73 | complete | Type-analysis and generated-code tests cover widening/narrowing at assignment, call, return, and mixed arithmetic boundaries. |
| 74 | complete | Positive and negative division cases fix Ruby integer division versus PHP/Perl floating division. |
| 75 | complete | Positive and negative operands fix floor remainder for Ruby/Perl and truncating remainder for PHP. |
| 76 | complete | Side-effect traces cover left-to-right operands and arguments; emitter tests cover index/place evaluation without duplicating expressions. |
| 77 | complete | Side-effecting right operands prove short-circuit behavior. |
| 78 | complete | Ruby's explicit nil/false truthiness lowering and PHP/Perl rejection of implicit truthiness are covered alongside absence checks. |
| 79 | complete | Shared-array mutation, debug positive/negative bounds, release omission of compiler checks, and index assignment are covered across semantic, compiler, and backend tests. |
| 80 | complete | Runtime/backend tests cover `__proto__`, `constructor`, `toString`, empty and Unicode keys, mutation, and located missing-key failure. |
| 81 | complete | Struct construction and type validation cover prototype-sensitive fields, duplicate/unknown fields, Unicode-safe target naming, and constructor/method boundaries. |
| 82 | complete | Empty/single/inclusive ranges, negative starts, while loops, break, continue, and nesting are divided between adapter, LIR, and host-semantic tests. |
| 83 | boundary complete | Tests assert typed codes, primary spans, suggestion spans/applicability/replacements, and registry metadata rather than snapshotting prose. A versioned external diagnostic protocol may add golden structured fixtures; until then, freezing every message would obstruct clearer wording without protecting semantics. |
| 84 | complete | Node's standard Source Map consumer resolves Unicode input back to original UTF-16 coordinates. |
| 85 | complete | Browser E2E checks a generated bounds location and an unset-uniform shader location in the accessible runtime overlay; nil/division location helpers are covered at the generated-runtime boundary. |
| 86 | complete | Asset tests cover escaping symlinks, spaces, fragment/query/percent delimiters, Unicode/case normalization, and file-directory prefix conflicts. |
| 87 | complete | GLSL tests cover scalar/bool, vec2–4, mat2–4, texture, all reflected attributes/varyings, and automatic uniforms. |
| 88 | complete | Browser conformance fixes the contract that debug rejects an unset active user uniform while release leaves WebGL's default. |
| 89 | complete | A decoded 1×1 image is sampled through a reflected texture shader and compared from a real WebGL framebuffer. |
| 90 | complete | Browser conformance exercises CSS resize with explicit DPR and a real `WEBGL_lose_context` transition; hostile runtime tests cover observer cleanup and restoration failure. |
| 91 | complete | Scheduled/manual Firefox and WebKit portability smoke runs are non-blocking; Chromium/SwiftShader remains the deterministic blocking baseline. |
| 92 | deferred | Hosted runners expose software rendering, not a stable attestable physical GPU. Re-enter with a trusted dedicated runner, recorded driver/GPU metadata, tolerant baselines, reset isolation, and an owner for driver upgrades. |
| 93 | complete | Framebuffer mismatch attaches actual, expected, and red-mask diff images. |
| 94 | complete | Baselines record OS, browser revision, renderer, viewport, runtime/shader ABI, and tolerance policy. |
| 95 | complete | CLI integration builds the same source in separate processes/directories and compares the entire artifact tree byte-for-byte. |
| 96 | complete | Deterministically generated integer programs check only overflow-safe metamorphic identities through compile and execution. |
| 97 | deferred | Cross-adapter source generation needs a canonical Neutral AST and one syntax-preserving printer per language; mutating one language's text cannot prove adapter equality. Re-enter after those four components exist, with shrinkable generated cases and L3 HIR comparison. |
| 98 | complete | Arbitrary source fuzzing reaches each adapter independently, while valid source continues through typed HIR, LIR, split, JavaScript, and GLSL. |
| 99 | complete | A >1 MiB input is rejected before parsing, a 4,096-statement source compiles, and a 4,096-deep dependency expression uses the iterative walker. |
| 100 | complete | Release CI packs the launcher and native packages, installs both into a fresh project on every supported platform, runs the CLI, and builds an example. |
| 101 | complete | Integration tests compile every example in debug and release and inspect the complete artifact contract. |

## Browser runtime and WebGL (102–140)

| # | Decision | Resolution |
| ---: | --- | --- |
| 102 | complete | `createRuntimeSession` creates independent sessions without changing the legacy global facade; session resources and stop handlers are isolated. |
| 103 | complete | Frame deltas are finite, non-negative, and capped by configurable `maxDeltaSeconds`. |
| 104 | complete | A fixed timestep with bounded catch-up steps is available and deterministic. |
| 105 | complete | Event-driven renders are coalesced to one scheduled frame. |
| 106 | complete | Visibility can pause scheduling and resets time state on resume. |
| 107 | complete | Resize observation, explicit DPR, validated browser DPR, and backing-buffer resize are implemented. |
| 108 | complete | Context loss prevents default restoration, stops rendering, and terminates deterministically when resources cannot be reconstructed. |
| 109 | complete | `nodeRemove` detaches nodes and releases resource references. |
| 110 | complete | Mesh and texture disposal invalidate handles and release owned GPU resources. |
| 111 | complete | Mesh/texture references keep resources alive until the last node/material reference is released. |
| 112 | complete | Handles are opaque branded identities; forged, stale, cross-session, and disposed handles are rejected. |
| 113 | complete | Counts, mesh bytes, decoded texture dimensions/bytes, shader programs, and actual GL limits are checked before allocation/upload. |
| 114 | complete | VAOs are cached by mesh and deterministic program attribute layout. |
| 115 | complete | Tier-1 geometry uses a geometrically growing reusable buffer and records uploads/growth. |
| 116 | complete | Program, array/element buffer, VAO, blend/depth, active texture, and 2D texture state share one cache. |
| 117 | complete | `exclusive` ownership is the default; callers can explicitly invalidate after external GL use or choose a per-frame `reset` policy. This does not claim to preserve caller-owned GL state. |
| 118 | complete | Frozen cumulative frame, batch, scene, shader, and GL-state statistics are exposed. |
| 119 | boundary complete | Circle tessellation adapts to projected transformed radius with hard bounds. Multiplying by DPR would double-count because drawing coordinates are backing pixels; add DPR only after an ADR changes the API to CSS logical pixels. |
| 120 | complete | Stroke width, cap, join, and bounded round/bevel/miter geometry are implemented. |
| 121 | boundary complete | Font, align, baseline, direction, max width, transforms, and DPR overlay resize have defined behavior. Waiting for webfont readiness requires a retained text queue capable of redraw; until then the default font remains browser-dependent by contract. |
| 122 | deferred | The current per-node `u_model` ABI cannot encode instance matrices or material batch keys. Re-enter after the shader ABI adds both plus equivalence/performance tests. |
| 123 | complete | Bounded basic nodes are frustum-culled; programmable shaders are conservatively retained. |
| 124 | complete | Opaque nodes write depth; transparent basic nodes sort back-to-front and temporarily disable depth writes. |
| 125 | deferred | Tier 1 is immediate, 3D is retained, and text is a DOM Canvas2D overlay, so arbitrary cross-layer ordering would be false. Re-enter with a unified render graph or offscreen text composition. |
| 126 | deferred | Lazy linking conflicts with startup reflection validation and deterministic shader diagnostics. Re-enter after structural artifact validation and link-time reflection become separate opt-in phases. |
| 127 | complete | User/automatic/draw uniforms track dirty state, avoid redundant uploads, and invalidate at external GL boundaries. |
| 128 | complete | Active samplers—not optimized-out declarations—are checked against texture-unit limits. |
| 129 | complete | External metadata is normalized without invoking getters, then active names, types, sizes, locations, and unexpected reflection entries are checked after linking. |
| 130 | complete | `getError` is scoped to uniform upload checks and drains stale errors before attributing a new failure. |
| 131 | deferred | Runtime cannot recover source lines absent a generated-GLSL-line-to-source-span table. Re-enter when the GLSL backend and artifact schema emit that table. |
| 132 | complete | Texture filtering, wrapping, flip-Y, premultiplication, and color-space options are normalized before use. |
| 133 | complete | Each image request receives an `AbortSignal`; disposal and session stop abort pending loads. |
| 134 | boundary complete | `stop`, white `placeholder`, and an error callback are implemented. Retry is deferred until count, backoff, cache interaction, abort, and terminal-error semantics are specified. |
| 135 | complete | Pointer, wheel, key, modifiers, buttons, deltas, repeat, and source coordinates are normalized into a richer frozen event. |
| 136 | complete | Keyboard events retain both logical `key` and physical `code`. |
| 137 | complete | Canvas focus-on-pointer and input `preventDefault` are independent validated policies. |
| 138 | complete | A frozen capability report includes GL limits, precision, renderer/vendor information where available, and runtime policies. |
| 139 | deferred | Arbitrary promises cannot be forcibly cancelled, and a timed-out setup could later call module-global APIs against a new session. Re-enter with a setup `AbortSignal` and session-bound API/token before adding timeout. |
| 140 | deferred | Program sharing needs a context+ABI+reflection+layout key, reference counts, and shared uniform invalidation. Re-enter after that state abstraction and a benchmark; sharing with the current per-registry dirty cache would skip required uploads. |

## CLI and development experience (141–158)

| # | Decision | Resolution |
| ---: | --- | --- |
| 141 | complete | Human, JSON, SARIF, and LSP diagnostics are emitted from structured data rather than parsed display text. |
| 142 | complete | `auto`, `always`, and `never` color policy is applied at the terminal boundary. |
| 143 | complete | Usage, parse, compile, I/O, and internal failures have distinct stable exit codes. |
| 144 | complete | `explain` reads title, description, producer, fixability, and introduction version from the diagnostic registry. |
| 145 | complete | `emit - --language …` reads stdin and stage output writes stdout; relative assets and external maps are rejected where a stream cannot represent them safely. |
| 146 | complete | Strict `polygl.toml` supports language, entry/output, modes/maps, packaging, and statically serializable runtime/resource options; CLI values override it and paths resolve relative to the config. |
| 147 | complete | `emit` supports deduplicated HIR, LIR, JavaScript, GLSL, and manifest selections. |
| 148 | complete | A single explicit template marker, conservative absolute base path, and recursive symlink-free public directory are supported with portable collision validation. |
| 149 | complete | `build --watch` preserves the last complete generation, retries failed configs/builds, and fingerprints source, config, template, public tree, and declared assets including changes during a build. |
| 150 | complete | `languages --json` exposes stable IDs, extensions, API version, and capabilities. |
| 151 | complete | Bash, zsh, fish, PowerShell completion output and a man page are generated. |
| 152 | complete | GET/HEAD parity, strong/weak/list ETags, 304, no-store development caching, and additional web asset MIME types are tested. |
| 153 | complete | The dev client reconnects with bounded exponential backoff and jitter. |
| 154 | complete | Temporary serve generations are owned by RAII values and removed after swap/drop and build failure. |
| 155 | complete | `serve --open` launches only after a successful loopback bind. |
| 156 | complete | BLAKE3 content names cover app, shaders, runtime, and external maps; schema-3 manifest entrypoints and full digests prevent filename guessing. Protocol-relative base paths, output/input containment, and map/hash consistency have adversarial tests. |
| 157 | complete | `--profile` reports pass timing, IR/resource counts, and output sizes. |
| 158 | deferred | Reproducibility is an unconditional build property, not an optional mode. Adding a flag would imply ordinary builds may contain absolute paths, time, or unstable order. The manifest and repeated builds instead enforce the stronger default guarantee. |

## CI, supply chain, documentation, and governance (159–207)

| # | Decision | Resolution |
| ---: | --- | --- |
| 159 | complete | Every third-party GitHub Action reference is pinned to a full commit SHA. |
| 160 | complete | Workspace tests run on current Ubuntu, macOS, and Windows images. |
| 161 | complete | MSRV 1.96.1, stable, and beta each check all targets/features. |
| 162 | complete | npm tests run on the published Node 20 minimum and the pinned development Node version. |
| 163 | complete | CI explicitly runs locked workspace tests with all features. |
| 164 | complete | Rust documentation treats warnings as errors, and ordinary workspace tests execute doctests. |
| 165 | complete | Pinned `cargo-deny` checks advisories, licenses, duplicate policy, and sources. |
| 166 | complete | Checksum-verified OSV-Scanner scans every committed Rust/npm lockfile under an expiring documented exception policy. |
| 167 | complete | Dependabot covers Cargo, runtime/browser npm, fuzz, stability probes, and GitHub Actions weekly. |
| 168 | complete | Pinned `cargo-llvm-cov` uploads workspace LCOV on pushes and pull requests. |
| 169 | deferred | There is no measured flaky test or test-time bottleneck that nextest would solve, and its execution differences would create a second test contract. Re-enter when CI timing/flaky records identify a target and define retries, partitioning, and JUnit retention. |
| 170 | deferred | The workspace forbids unsafe code and the requested span/validator/optimizer paths are safe logic; Miri adds no distinct memory-safety oracle there. Re-enter if unsafe/FFI is introduced or a strict-provenance issue is reproducible; property, fuzz, and sanitizer tests cover current risks. |
| 171 | complete | A pinned nightly scheduled job runs all parser and backend crates under AddressSanitizer. |
| 172 | complete | Scheduled isolated language-frontends and valid end-to-end pipeline fuzz targets use a pinned nightly/tool version and bounded timeouts. |
| 173 | complete | Pinned mutation jobs challenge path validation, effect/optimizer behavior, and capability-driven conformance selection. |
| 174 | complete | Compiler benchmark fixtures cover small, medium, nominal-large, shader-heavy, class-heavy, and error-heavy builds in clean output directories. |
| 175 | complete | A headless WebGL benchmark records setup/frame time and runtime statistics for 10,000 shapes, 256 nodes, 32 textures, and reflected uniform work. |
| 176 | complete | CI rejects runtime, application JavaScript, or GLSL output beyond calibrated raw-byte budgets. |
| 177 | complete | Release executable clean builds and independent application build processes are both compared byte-for-byte. |
| 178 | complete | Pull requests run pinned `cargo-semver-checks` for public workspace crates. |
| 179 | complete | Native archives sort portable entry names and fix mode, uid/gid, tar mtime, and gzip timestamp; reversed-input tests produce identical bytes. |
| 180 | complete | Release preflight generates and validates a CycloneDX 1.6 SBOM and includes it in checksums. |
| 181 | complete | Tag releases receive GitHub OIDC/Sigstore SBOM attestations over checksum-selected native archives. |
| 182 | complete | Cargo/npm publication scripts maintain validated per-package ledgers and skip an already published matching version, allowing a partial release to resume. |
| 183 | complete | macOS uses deployment target 11.0; release binaries are inspected for that floor, glibc <=2.39, or a valid Windows PE. |
| 184 | complete | Every native npm pair is installed and exercised in an empty project on its target OS/architecture. |
| 185 | complete | Preflight proves archive binaries and staged npm-native binaries have matching hashes before either is published. |
| 186 | boundary complete | npm uses short-lived OIDC trusted publishing. crates.io receives its token only in the environment-protected publish job; migrate it to crates.io trusted publishing when the repository/account policy supports the required multi-crate workflow. |
| 187 | complete | `CONTRIBUTING.md` documents pinned setup, generated files, test layers, ADR triggers, and adapter workflow. |
| 188 | complete | `SECURITY.md` defines supported releases, private reporting, compiler/dev-server/generated-site boundaries, and asset/untrusted-source threats. |
| 189 | complete | `CHANGELOG.md` follows a release-oriented Keep a Changelog structure. |
| 190 | complete | `ROADMAP.md` separates committed direction from non-goals such as full source-language compatibility and a WebGL 1 fallback. |
| 191 | complete | Contributor Covenant behavior, enforcement, and path ownership are recorded in the Code of Conduct and CODEOWNERS. |
| 192 | complete | Five routed issue forms cover compiler, adapter, runtime/WebGL, language proposal, and diagnostic proposal reports. |
| 193 | complete | The pull-request template checks every cross-layer contract and generated/documented surface. |
| 194 | complete | The architecture tutorial follows one program through adapters, HIR, typed HIR, LIR, split, JS/GLSL, packaging, and runtime validation. |
| 195 | complete | The support matrix distinguishes guaranteed tool versions, blocking Chromium/SwiftShader, non-blocking Firefox/WebKit, and unsupported real-GPU claims. |
| 196 | complete | CLI and deployment documentation explain Source Map path/source disclosure and `sourcesContent` policy. |
| 197 | complete | The deployment guide covers base paths, immutable hashed caching, HTML/manifest caching, CSP, MIME types, maps, and integrity verification. |
| 198 | complete | The resource lifecycle guide specifies node, mesh, texture, material, shader, session, abort, reference, and context-loss ownership. |
| 199 | complete | Ruby, PHP, and Perl include the same triangle and rotating-cubes subjects for direct comparison. |
| 200 | complete | Dedicated examples cover texture lifecycle, custom mesh terrain, input, and a located runtime bounds failure. |
| 201 | complete | The ADR index records accepted/superseded status and explicit replacement links. |
| 202 | complete | API, capability, and diagnostic reference pages are generated from runtime/builtin, adapter, and diagnostic registries. |
| 203 | complete | HIR/LIR documentation states that human dumps are deterministic test/debug output, not a stable serialization API; schema constants govern internal compatibility. |
| 204 | complete | The SemVer policy separately governs user/compiler behavior, runtime/shader ABI, adapter API, IR schemas, and feature versions. |
| 205 | boundary complete | A dated machine-specific baseline publishes compiler latency, artifact budgets/sizes, WebGL frame/setup time, and draw/upload counters. Portable peak RSS and cross-machine pass/fail comparisons are deferred until a normalized collector and representative stable runner exist. |
| 206 | complete | The debugging guide shows how to retain/read generated JS, GLSL/reflection, Source Maps, structured diagnostics, and runtime overlays. |
| 207 | complete | The spike policy marks each experiment active, accepted, superseded, or frozen and defines CI/maintenance/promotion requirements. |
