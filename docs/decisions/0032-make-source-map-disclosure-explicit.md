---
---

# 0032: Make Source Map disclosure explicit

- Status: Accepted
- Date: 2026-08-12
- Supersedes: [ADR 0015](0015-pair-source-maps-with-runtime-locations.md) for packaging policy

## Context

ADR 0015 correctly tied Source Map mappings and runtime locations to the same
source spans, but required every debug and release artifact to publish an
external map containing the complete original source. A public static build
could therefore disclose private paths, comments, literals, and source text
without an explicit packaging decision.

Runtime error locations do not require a browser Source Map consumer. The
compiler can preserve their shared span provenance while allowing deployment
policy to control which optional debugging artifact is published.

## Decision

Keep Source Map v3 mappings and runtime location tables derived from the same
validated spans. Support `none`, `external`, and `inline` map modes, and control
`sourcesContent` independently. The CLI defaults debug builds to an external
map without source content and release builds to no map. The loopback-only
development server uses an external map with source content.

Normalize CLI source names to project-relative `/`-separated paths. When a
source is outside the project working directory, retain only its basename so
an absolute path or user directory cannot enter distributable artifacts.

## Consequences

Runtime overlays remain source-located even when Source Maps are omitted.
Deployments opt into the privacy and size cost of maps and embedded sources,
while local development retains a convenient default. Consumers of the backend
library keep the previous external-map behavior unless they select another
mode.
