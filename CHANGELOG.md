# Changelog

All notable changes are documented here. The format follows Keep a Changelog,
and releases follow the compatibility policy in `docs/semver.md`.

## Unreleased

### Added

- Public compiler orchestration, structured diagnostics, profiling, configured
  deployment builds, warning policy, and development workflows.
- Multi-session runtime lifecycle, resource limits/disposal, reflected shader
  validation, rendering statistics, and explicit WebGL state ownership.
- Capability-driven adapter/conformance manifests, Host semantic execution,
  browser portability smoke tests, fuzzing, coverage, mutation, benchmarks, and
  release supply-chain attestations.

### Changed

- Build output is transactionally staged and packaged with reproducible,
  content-addressed metadata and explicit Source Map disclosure.
- Host LIR uses effect-aware reachability and deterministic symbol/dependency
  analysis.

### Security

- Asset, URL, output, JavaScript-object, runtime-metadata, and development-server
  trust boundaries reject ambiguous or escaping inputs.

## 0.1.0 - 2026-07-29

### Added

- Initial Ruby, PHP, and Perl adapters, shared HIR/LIR compiler, JavaScript and
  GLSL backends, WebGL 2 runtime, CLI, conformance suite, and native npm release.
