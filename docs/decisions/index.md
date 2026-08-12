---
title: Architecture decision records
permalink: /decisions/
---

# Architecture decision records

An accepted ADR is normative until another ADR explicitly supersedes it.
Proposed records are not contracts. A superseded record remains for history and
links to its replacement.

| ADR | Status | Superseded by |
| --- | --- | --- |
| [0000: Template](0000-template.md) | Proposed | — |
| [0002: Two-level structured IR](0002-two-level-structured-ir.md) | Accepted | — |
| [0006: Behavior-first conformance](0006-behavior-first-conformance.md) | Accepted | — |
| [0008: Lower Ruby truthiness explicitly](0008-lower-ruby-truthiness-explicitly.md) | Accepted | — |
| [0009: Type inference strategy](0009-type-inference-strategy.md) | Accepted | — |
| [0011: Third-language Perl parser](0011-third-language-perl-parser.md) | Accepted | — |
| [0012: Common Core contract](0012-common-core-contract.md) | Accepted | — |
| [0013: Use Prism for Ruby](0013-use-prism-for-ruby-parsing.md) | Accepted | — |
| [0014: Separate builtin registry](0014-separate-builtin-registry.md) | Accepted | — |
| [0015: Pair maps with locations](0015-pair-source-maps-with-runtime-locations.md) | Superseded | [0032](0032-make-source-map-disclosure-explicit.md) |
| [0016: Encode remainder direction](0016-encode-remainder-direction.md) | Accepted | — |
| [0017: Bind runtime operations](0017-bind-runtime-operations-to-an-active-session.md) | Accepted | — |
| [0018: Embed generated runtime](0018-embed-a-generated-runtime-bundle.md) | Accepted | — |
| [0019: Reflected fixed shader ABI](0019-use-a-reflected-fixed-shader-abi.md) | Accepted | — |
| [0020: Prove GPU integer divisors](0020-require-provably-nonzero-gpu-integer-divisors.md) | Accepted | — |
| [0021: Reflected shader data](0021-package-shaders-as-reflected-data.md) | Accepted | — |
| [0022: WebGL plus text overlay](0022-combine-batched-webgl-with-a-text-overlay.md) | Accepted | — |
| [0023: Local stateful dev server](0023-keep-the-development-server-local-and-stateful.md) | Accepted | — |
| [0024: Resolve methods after typing](0024-resolve-instance-methods-after-type-inference.md) | Accepted | — |
| [0025: Adopt Mago for PHP](0025-adopt-mago-for-php-parsing.md) | Accepted | — |
| [0026: Centralize adapter conventions](0026-centralize-language-neutral-adapter-conventions.md) | Accepted | — |
| [0027: Session-owned Tier 2 handles](0027-use-session-owned-tier-2-handles.md) | Accepted | — |
| [0028: Native CLI build matrix](0028-publish-native-cli-packages-from-one-build-matrix.md) | Accepted | — |
| [0029: Coordinated Rust crate versions](0029-version-rust-crates-for-coordinated-publication.md) | Accepted | — |
| [0030: Generate dependency notices](0030-generate-release-license-notices-with-cargo-about.md) | Accepted | — |
| [0031: Reproducible release preflight](0031-gate-publication-behind-a-reproducible-preflight.md) | Accepted | — |
| [0032: Explicit Source Map disclosure](0032-make-source-map-disclosure-explicit.md) | Accepted | — |
| [0033: Fail-closed runtime lifecycle](0033-make-runtime-lifecycle-fail-closed.md) | Accepted | — |

New records copy 0000, state status/date/context/decision/consequences, and link
any replaced decision in both directions. Changing Common Core meaning, a
public schema/ABI, resource ownership, security boundary, or release trust model
requires an ADR before implementation is considered complete.
