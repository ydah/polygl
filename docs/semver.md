---
title: Compatibility and versioning
permalink: /semver/
---

# Compatibility and versioning

Package versions use SemVer, but several machine contracts advance separately
so an incompatibility is rejected rather than inferred from one package number.

| Contract | Marker | Compatibility rule |
| --- | --- | --- |
| Public Rust crates and CLI | package SemVer | incompatible public API, flags, exit codes, or documented Common Core behavior require a major release after 1.0 |
| Generated Host module/runtime | `RUNTIME_ABI_VERSION` | exact match at browser startup |
| Shader data/runtime | `SHADER_ABI_VERSION` | exact match before link/reflection |
| Language adapter trait | `ADAPTER_API_VERSION` | exact match before any future dynamic invocation |
| HIR/LIR/builtin structure | schema constants | exact match for provenance/cache/tool consumers; human dumps are not stable serialization |
| Capabilities | names such as `maps-v1` | consumers match the complete versioned tag; semantic changes create a new tag |
| Artifact manifest | `schemaVersion` | consumers reject unknown major schema rather than guessing fields |

Before 1.0, package minor releases may change public source APIs, but release
notes must identify the change and `cargo-semver-checks` reports it in review.
Patch releases do not intentionally change accepted Common Core meaning or any
ABI/schema marker.

Adding a backward-compatible diagnostic, adapter capability, optional config
field, or runtime API is minor. Fixing behavior that contradicted an existing
normative contract may be a patch, with regression evidence and a changelog
security/fixed entry as appropriate.

Generated artifacts are self-describing but not forward-compatible by default.
Deploy compiler output and its embedded runtime from the same complete build;
do not mix files from releases even if package versions appear close.
