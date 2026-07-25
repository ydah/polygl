# 0017: Bind runtime operations to an active session

- Status: Accepted
- Date: 2026-07-25

## Context

Generated ES modules import the runtime as a namespace and call operations such
as `background`, `circle`, and `random` as plain functions. Those operations
nevertheless need shared canvas, renderer, input, clock, and random-generator
state. Passing that state through every generated call would change the
BuiltinTable signatures and leak backend plumbing into Common Core.

## Decision

The browser runtime owns one active `RuntimeSession`. The normal entry is
`start(() => import("./app.js"))`: `start` stops the previous session, creates a
WebGL2 renderer and deterministic state for the supplied canvas, makes that
session active, and only then evaluates the generated module and invokes its
lifecycle functions. This ordering also supports runtime calls in module-level
constant initializers. Exported operations delegate to the active session and
fail clearly when called before `start`.

Overlapping starts are rejected until module loading and asynchronous setup
finish. This prevents a suspended setup from resuming against a newer active
session.

The session receives scheduling, canvas, WebGL context, document, seed, and
error-reporting dependencies through options so lifecycle behavior remains
deterministic under tests.

## Consequences

Generated calls retain the canonical BuiltinTable signatures and the runtime
state has one explicit owner. Tests can replace browser dependencies without a
global DOM. A single loaded runtime module intentionally drives one active
canvas at a time; starting another session stops the earlier animation loop.
