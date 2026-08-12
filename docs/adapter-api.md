---
---

# Language adapter API

`ADAPTER_API_VERSION` is the compatibility version of the public trait and
lowering contract. Artifact manifests record it together with versioned feature
tags; a future plugin boundary must compare it before invoking an adapter.

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

The API also owns the language-neutral adapter conventions:

- `parse_annotation_type` and `is_portable_identifier` implement the portable
  `@pgl` type grammar;
- `canonical_entry_kind` recognizes `setup`, `frame`, `on_event`, and named
  shader entries;
- `constructor_function_name` names generated struct constructors; and
- `vector_constructor_size` recognizes canonical vector construction.

Source comment attachment, parser traversal, source-language aliases,
temporary-name hygiene, and semantic expansion remain inside each adapter.
See the [post-PHP boundary review](adapter-boundary-review.md) and
[ADR 0026](decisions/0026-centralize-language-neutral-adapter-conventions.md).

Source identifiers retain their parser-provided UTF-8 spelling and are
case-sensitive. PolyGL deliberately does not normalize them: NFC and NFD
spellings remain distinct so compilation cannot silently merge two bindings
that the source parser kept separate. Backends encode the original bytes into
collision-free target identifiers. The narrower identifier grammar inside
portable `@pgl` directives starts with `_` or an alphabetic Unicode scalar and
continues with `_` or alphanumeric scalars; combining marks, format controls,
emoji, and whitespace are rejected. Adapters must not apply locale-sensitive
case conversion.

The trait is `Send + Sync` and has no generic methods, so
`Box<dyn LanguageAdapter>` is supported by the static v1 registry and a future
plugin boundary.
