---
---

# Language adapter authoring guide

This guide is the required workflow for adding a source language to PolyGL.
An adapter parses one UTF-8 file and lowers a documented Common Core subset to
source-spanned HIR. It does not emulate the complete source language.

Read these contracts before writing lowering code:

- [Common Core](common-core.md)
- [Adapter API](adapter-api.md)
- [HIR](hir.md)
- [Type system](type-system.md)
- [Conformance](conformance.md)
- [Diagnostic codes](errors.md)
- [Adapter boundary review](adapter-boundary-review.md)

## 1. Select and characterize a parser

Prefer a maintained typed parser crate, then a maintained tree-sitter grammar.
Record an ADR covering:

- maintenance and release activity;
- license and transitive license compatibility;
- supported source-language versions;
- complete half-open byte spans;
- comments or DocBlocks needed by `@pgl`;
- parse-error recovery and diagnostics;
- MSRV and dependency/build cost; and
- a fallback if the selected parser becomes unusable.

Pin parser versions when upstream MSRV or API churn can silently change the
workspace. Add characterization tests for one valid program, exact node and
comment spans, one annotation comment, and one syntax error before implementing
lowering. Parser types must not leave the adapter crate.

## 2. Add the crate and adapter shell

Generate a standalone shell, or create the equivalent crate inside the
workspace:

```console
polygl new-adapter example -o crates/polygl-adapter-example
```

The command refuses to overwrite an existing path and removes a partial new
directory if writing fails. It generates a compilable adapter stub, metadata
and diagnostic contract tests, a parser/mapping checklist, and a README. Add
the adapter to compiler orchestration only after its shell compiles, then
confirm that `polygl languages` shows its stable identifier and extension. The
generated implementation has this shape:

```rust
use polygl_adapter_api::{FeatureTag, LanguageAdapter, LowerCtx};
use polygl_hir::Module;
use polygl_span::{Diagnostics, SourceFile};

pub struct ExampleAdapter;

impl LanguageAdapter for ExampleAdapter {
    fn id(&self) -> &'static str {
        "example"
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["example"]
    }

    fn lower(
        &self,
        source: &SourceFile,
        context: &mut LowerCtx<'_>,
    ) -> Result<Module, Diagnostics> {
        todo!("parse and lower one source file")
    }

    fn capabilities(&self) -> &'static [FeatureTag] {
        &[FeatureTag::Core]
    }
}
```

Capabilities are promises tested by conformance selection. Advertise a tag
only after its syntax, diagnostics, and tests are complete. Resolve graphics
builtins through `LowerCtx::resolve_builtin`; never copy numeric builtin IDs.

Use the helpers in `polygl-adapter-api` for portable annotation types,
canonical entry names, vector constructors, and generated class constructor
names. Keep parser traversal and source semantics in the adapter.

## 3. Write the syntax mapping first

Create `docs/adapters/<language>.md` before broad lowering. Its mapping table
must cover:

- functions, required positional parameters, and explicit return;
- `setup`, `frame`, `on_event`, `vertex_*`, and `fragment_*`;
- literals, locals, assignment, constants, calls, and fields;
- arithmetic, division, remainder, concatenation, and comparison;
- boolean conditions, absence values, and absence tests;
- `if`/`else`, `while`, ascending `for`, loop control, and any collection
  sugar;
- arrays, string-keyed maps, index reads, and index writes;
- the struct-like class subset;
- the exact `@pgl` comment form and adjacency rule; and
- every advertised `FeatureTag`.

Explicitly state deliberate semantic differences. In particular, decide how
the language maps division, remainder direction, truthiness, string
concatenation, strict equality, and `NilCheck`. Do not defer these choices to
JavaScript behavior.

## 4. Lower in dependency order

Implement and test in this order:

1. parser errors and span conversion;
2. literals and direct variable names;
3. first-write `Let`, later `Assign`, and lexical scopes;
4. arithmetic and source-specific operators;
5. calls and builtin resolution;
6. `if`, `while`, range `for`, return, break, and continue;
7. arrays, maps, indexing, and evaluate-once collection sugar;
8. constants and annotation constraints;
9. classes, fields, construction, and static method dispatch; and
10. entry points and shader capabilities.

Every HIR node must carry a span from the original source. Treat the HIR
evaluation model as normative. If source syntax evaluates an expression more
often than HIR does, either introduce an evaluate-once temporary when that
preserves meaning or reject the form with a rewrite suggestion.

Generated temporaries must be hygienic. Walk all source identifiers before
lowering or use another parser-backed collision strategy; checking only locals
seen so far is insufficient because a later declaration can capture a
generated name.

HIR scopes are lexical. If the source language has function-scoped or
non-block-scoped locals, document that a first assignment in a nested HIR block
does not escape. A user can initialize the local in the containing block when
it is needed later.

## 5. Apply annotations and native types

Parse the language-specific directive wrapper, then call
`parse_annotation_type` for its portable type spelling. Associate a directive
only with the intended adjacent declaration and report every malformed,
unknown, non-adjacent, or unused directive as E0314.

For empty arrays and maps, consult the declaration's annotation before choosing
the HIR aggregate kind. Native source-language hints may provide the same
constraint. Consume a redundant matching directive; report a native/directive
conflict precisely instead of leaving the directive to become an “unused”
error. `void` is valid only in a return position.

## 6. Keep classes structural

The v1 class subset has fixed fields, one constructor, and instance methods.
Lower construction to a generated function returning `ExprKind::Struct`.
Attach method templates to `StructDef`, give each a typed `self` first
parameter, and emit receiver calls as `Callee::Method` with the receiver as the
first argument. Shared type analysis resolves the receiver and specializes the
static function.

Reject inheritance, interfaces or traits, static members, visibility
semantics, reflection, dynamic members, and general constructor side effects
as E0203. A constructor must not read `self` before the struct value exists.

## 7. Design diagnostics as part of the feature

Unsupported syntax must not become a parser error, panic, silent omission, or
user-function fallback. Produce a structured diagnostic with:

- the stable code required by [errors.md](errors.md);
- a non-empty primary span on the source construct;
- a specific message explaining the portability boundary; and
- a safe machine replacement or a concrete human rewrite suggestion.

Use E0202 for escaping/general closure values and E0203 for excluded class
features. Use E0301 for non-boolean conditions, E0302 for loose equality, E0303
for type conflicts, and E0314 for directive syntax or placement. Add at least
ten language-specific rejection cases and assert codes, spans, and
replacements for operators that have safe edits.

## 8. Integrate the compiler and conformance layers

Add extension-based CLI selection and at least one end-to-end browser artifact
test. Then add the language to:

- every applicable L1 case as `main.<extension>`;
- a separate L2 `<language>.hir` snapshot for every case;
- direct `compare_neutral_hir` calls for every supported Neutral case; and
- browser rendering so every language framebuffer is checked against the same
  renderer-keyed L1 baseline.

Keep non-Neutral differences out of L3 and test them explicitly. Division is
the standard example: two adapters may intentionally emit different HIR while
both obey their source-language contract.

## 9. Review the adapter boundary

After the second real adapter, duplicated logic must be classified rather than
blindly shared:

- move Common Core/HIR policy into `polygl-adapter-api` or another owning
  shared crate;
- keep parser-specific syntax, trivia, diagnostics, and semantic expansion in
  the adapter; and
- record architectural ownership changes in an ADR.

Do not introduce source-language flags into HIR, type analysis, LIR, backends,
or the runtime. A new language should normally add an adapter, documentation,
fixtures, and orchestration registration—not conditional behavior throughout
the compiler.

## Completion checklist

- Parser decision and characterization tests are committed.
- Mapping documentation covers every accepted construct and difference.
- All advertised capabilities have positive and negative tests.
- Every rejection has a source span and suggestion.
- At least ten language-specific diagnostic cases pass.
- CLI build, check, and typed HIR dump accept the extension.
- `polygl languages` reports the adapter identifier and extension.
- L1 behavior, L2 snapshots, and applicable L3 direct comparisons pass.
- Intentional non-Neutral differences have explicit tests.
- `cargo fmt`, Clippy with warnings denied, workspace tests, runtime tests,
  generation freshness, Rust conformance, and browser conformance pass.
- Public API and behavior changes are reflected in the corresponding
  specifications.
