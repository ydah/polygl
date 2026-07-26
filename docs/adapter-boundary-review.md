# Adapter boundary review after PHP

This M4 review records which concerns discovered while adding PHP belong in
the shared compiler and which must remain in a source-language adapter.

| Concern | Owner | Resolution |
|---|---|---|
| Portable `@pgl` type spellings | `polygl-adapter-api` | Shared parser and identifier validation |
| Canonical entry names | `polygl-adapter-api` | Shared recognition; language aliases wrap it |
| Generated class constructor name | `polygl-adapter-api` | Shared `Class::new` naming convention |
| Canonical `vec2`/`vec3`/`vec4` names | `polygl-adapter-api` | Shared recognition |
| Builtin lookup | `LowerCtx` | Existing canonical resolver remains shared |
| DocBlock/comment adjacency | each adapter | Parser trivia and comment rules differ |
| Division, remainder, truthiness, equality | each adapter | Source semantics must be made explicit in HIR |
| Loop and collection sugar | each adapter | Evaluation timing and accepted syntax differ |
| Temporary-name hygiene | each adapter | Requires a complete source-parser name walk |
| Class member discovery | each adapter | CST shapes and source class conventions differ |
| Type inference and method resolution | `polygl-types` | Existing language-neutral analysis remains shared |
| L1/L2/L3 comparison | conformance runner | Existing shared behavior and HIR checks now cover both languages |

The review found no PHP parser type or PHP-specific semantic flag leaking into
HIR, type analysis, LIR, either backend, or the runtime. The only shared-layer
change needed was extraction of already-identical conventions. This keeps the
adapter boundary narrow while preventing a third implementation from copying
Common Core policy.
