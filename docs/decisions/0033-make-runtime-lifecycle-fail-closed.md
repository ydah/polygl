---
---

# 0033: Make runtime lifecycle transitions fail closed

- Status: Accepted
- Date: 2026-08-12

## Context

Browser scheduling and WebGL resources have failure modes that do not appear
in a continuously visible page: animation timestamps can jump after tab
suspension, input can request many redundant renders, CSS and drawing-buffer
sizes can diverge, and a restored WebGL context has lost all GPU objects.
Retained scene resources also need a lifetime shorter than the whole session.

## Decision

Frame deltas are capped at a configurable positive maximum, with 0.1 seconds as
the default. Input and resize redraws share a single pending animation frame.
CSS/DPR resize tracking is opt-in and injectable for deterministic tests.

Context loss suspends scheduling. Context restoration is terminal for the
current session and tells the caller to restart; the runtime does not pretend
that invalidated GPU objects remain usable. Scene handles are frozen facades
whose resources live in private weak maps. Nodes hold counted mesh and texture
references, and explicit disposal is rejected until those references are
released.

Adding the disposal operations changes both the builtin schema and generated
JavaScript/runtime contract, so their compatibility versions advance together.

## Consequences

Large wall-clock gaps cannot destabilize a simulation, event storms perform at
most one extra render per display frame, and high-DPI resizing has an explicit
policy. Programs can reclaim retained resources without waiting for session
shutdown. A restored context requires application restart rather than partial,
driver-dependent recovery; transparent resource rehydration may be added later
only with a complete replayable resource model.
