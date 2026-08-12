# Security policy

## Supported versions

Security fixes are provided for the latest released minor version and the
current `main` branch. Older releases may be useful for reproducing a report,
but are not maintained after a fixed release is available.

## Reporting a vulnerability

Do not open a public issue for a suspected vulnerability. Use the repository's
[private vulnerability reporting](https://github.com/ydah/polygl/security/advisories/new)
form. Include the affected version/commit, operating system, a minimal input,
the observed impact, and whether the input can be shared with maintainers.

Maintainers will acknowledge a complete report within seven days, reproduce or
reject it with evidence, and coordinate disclosure after a fix is available.
There is currently no paid bug-bounty program.

## Security boundaries

PolyGL compiles source programs and packages referenced assets. Source code,
parser input, configuration, templates, and public-directory contents must be
treated as untrusted. Resource budgets, checked spans, canonical asset
containment, symlink rejection, portable collision checks, and transactional
publication are security boundaries; a panic, out-of-tree read/write, or
resource-limit bypass is reportable.

`polygl serve` is a local development server. It binds to loopback, validates
`Host` and WebSocket origin, bounds connections and request bytes, and is not a
production or multi-user server. Do not expose it through a public proxy.

Generated sites execute the author's program in a browser and may fetch assets
the author named. PolyGL does not sandbox that program from its hosting origin.
Deploy it with an origin and Content Security Policy appropriate for untrusted
JavaScript. Source Maps and `sourcesContent` can disclose source paths, comments,
literals, or the complete input and are off by default for release builds.

The runtime validates generated module metadata and shader reflection, but it
does not claim isolation from other scripts on the same page. Exclusive WebGL
state ownership is the default. Callers that mutate the same context must use
the documented external-state boundary.
