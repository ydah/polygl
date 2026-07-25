# Language adapter API

Adapters implement the object-safe `LanguageAdapter` trait:

- `id` returns the stable lowercase language identifier;
- `file_extensions` declares recognized entry-file suffixes;
- `lower` parses one `SourceFile` and returns source-spanned HIR or structured
  diagnostics; and
- `capabilities` declares FeatureTags used by conformance case selection.

`LowerCtx` exposes canonical builtin lookup through `BuiltinResolver`. The API
crate does not depend on the registry; `polygl-builtins` implements the
resolver for its canonical `BuiltinTable`, and `polygl-core` re-exports it for
the orchestration facade. This keeps the dependency direction adapter-api →
HIR/span and builtins → adapter-api, avoiding a cycle when core assembles
adapters and shared analysis.

An adapter must resolve builtin names instead of constructing or persisting raw
IDs. A missing canonical name is a compiler configuration error, not a
source-language fallback.

The trait is `Send + Sync` and has no generic methods, so
`Box<dyn LanguageAdapter>` is supported by the static v1 registry and a future
plugin boundary.
