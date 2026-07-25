# Command-line interface

The M1 CLI accepts one Ruby source file and runs the shared compiler pipeline.

```text
polygl build <source.rb> [-o <directory>] [--debug | --release]
polygl check <source.rb>
polygl dump-hir <source.rb>
```

`build` defaults to `dist` and debug mode. It writes:

- `index.html`, whose module bootstrap activates the runtime before dynamically
  importing the generated program;
- `app.js` and `app.js.map`;
- `runtime.js`, embedded in the CLI binary from the tested TypeScript runtime.

Debug output includes source-located array, vector, matrix, and nil checks.
Release output removes those checks but retains the source map. Build output
must be served over HTTP because browsers restrict module imports from local
`file:` pages.

`check` performs parsing, Common Core lowering, type inference, and
monomorphization without writing artifacts. `dump-hir` performs the same checks
and prints fully typed HIR. Adapter and type errors use stable diagnostic codes,
source excerpts, and suggestions where required.
