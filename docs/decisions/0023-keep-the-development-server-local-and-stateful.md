---
---

# 0023: Keep the development server local and stateful

- Status: Accepted
- Date: 2026-07-25

## Context

The watch workflow needs to rebuild a single source file, reload connected
browsers after success, and display compiler diagnostics after failure. A failed
rebuild must not destroy the last runnable artifacts. General-purpose HTTP,
filesystem-watch, and WebSocket stacks would add substantial dependencies to
the compiler binary for a deliberately small local workflow.

## Decision

Bind the development server to `127.0.0.1`, serve only canonical files below
the active private generation, and poll both source metadata and a full-content
hash at a short fixed interval. Implement the RFC 6455 handshake and
server-to-client text frames needed by the injected development client.
WebSocket upgrades require the server's exact loopback Origin, have a fixed
connection limit, and use reader tasks to process control frames and reap
disconnects.

Build each revision into a new temporary generation before notifying clients. A
successful build atomically swaps the in-memory active generation and
broadcasts a reload; outstanding HTTP requests retain their generation until
completion. A failed build retains the last successful generation. The server
decorates `index.html` in memory with the development client and, when needed,
an HTML-escaped diagnostic text node, then broadcasts the same diagnostic to
already connected pages.

## Consequences

`serve --watch` has no new runtime dependencies and works with editor
save-by-replace behavior. Reading and hashing the whole source every 150 ms is
more I/O than metadata-only polling, but prevents same-size/coarse-timestamp
edits from being missed. It watches the one source file supported by the current
compiler rather than a general dependency graph. The WebSocket application
protocol remains one-way; expanding it beyond reload/error messages will
require a fuller state machine.
