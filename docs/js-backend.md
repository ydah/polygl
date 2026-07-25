# JavaScript backend

`polygl-backend-js` emits readable ES2020 modules for the Host and Shared
portions of LIR. The generated module imports the configured runtime namespace
and exports canonical `setup`, `frame`, and `on_event` entry functions. GPU-only
constants, functions, and shader entries are left for the GLSL backend.

The emitter preserves Common Core evaluation order. Integer addition,
subtraction, negation, multiplication, division, remainder, and range
increments retain signed 32-bit behavior; inclusive ranges also terminate
correctly at `i32::MAX`. Floor-directed remainder is implemented independently
of JavaScript's truncation-directed `%`. Generated identifiers are escaped when
a source name is reserved or unsafe in JavaScript, while constant and function
names use separate namespaces so a local may shadow a source constant. Nested
blocks preserve parameter, loop-variable, and local shadowing from LIR.

## Artifacts and source maps

`JavaScriptBackend::generate` accepts the LIR module and every `SourceFile`
referenced by its spans. It returns JavaScript and an external Source Map v3
document. Mappings use zero-based UTF-16 columns as required by browser source
map consumers, while `sourcesContent` embeds the exact original text for
DevTools. Missing, duplicate, or invalid source spans are emission errors
rather than silently degrading to generated locations.

The default output pair is `app.js` and `app.js.map`; both the output name and
runtime module specifier are configurable.

## Debug and release modes

Debug is the default mode. Each checked operation refers to a frozen location
entry containing the original file, one-based line and column, and byte range.
The emitter uses these runtime hooks:

- `checkedIndex(base, index, location)` for collection reads;
- `checkIndex(base, index, location)` before collection writes; and
- `requireNonNil(base, location)` before field reads or writes.

Collection writes use an internal expression wrapper so the base and index are
evaluated once, validation happens before the right-hand side, and source
evaluation order is unchanged. Generated integer division and remainder errors
also carry the same location as `error.polyglLocation`. Release mode emits
direct JavaScript access and omits the location table and these checks. Source
maps remain available in both modes.

Unset-uniform validation belongs to the shader ABI and GPU split introduced in
M2; it uses the same source-location contract rather than being approximated
in Host JavaScript.
