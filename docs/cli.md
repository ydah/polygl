# Command-line interface

The CLI accepts one source file and runs the shared compiler pipeline.

```text
polygl build <source.rb> [-o <directory>] [--debug | --release]
polygl serve <source.rb> [--port <port>] [--watch]
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

`serve` builds debug artifacts into a private temporary generation, binds only
to `127.0.0.1` (port 4173 by default), and serves it with caching disabled.
`--watch` hashes the source at a short interval and injects a same-origin
WebSocket client. Successful rebuilds atomically switch the active generation
and reload connected pages. Failed rebuilds preserve the last good generation
and display HTML-escaped source diagnostics in a browser overlay. A page opened
during a failing build receives the same current diagnostic. Without `--watch`,
an initial compiler error exits instead of starting a server that cannot
recover.

`check` performs parsing, Common Core lowering, type inference, and
monomorphization without writing artifacts. `dump-hir` performs the same checks
and prints fully typed HIR. Adapter and type errors use stable diagnostic codes,
source excerpts, and suggestions where required.
