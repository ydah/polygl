# S-3: WebGL2 screenshot stability

- Date: 2026-07-25
- Time box: 0.5 day
- Result: **Conditional go; first hosted run pending**

## Question

Does pinned Chromium running through SwiftShader produce stable WebGL2
screenshots for deterministic input?

## Probe

`spikes/webgl-stability` uses Playwright 1.57.0 and launches Chromium with the
current Chromium-documented flags:

```text
--use-gl=angle
--use-angle=swiftshader-webgl
--enable-unsafe-swiftshader
```

The test launches three independent Chromium processes, renders one
antialias-free WebGL2 triangle in each, verifies every unmasked renderer
contains `SwiftShader`, takes a canvas screenshot, and compares all three
SHA-256 digests byte-for-byte.

Run it with:

```console
pnpm --dir spikes/webgl-stability test
```

Observed renderer:

```text
ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device (LLVM 10.0.0) (0x0000C0DE)), SwiftShader driver)
```

All three screenshots produced:

```text
94006ea8acf4f36b06c1c436835466efe07c31a9175084f697f0f11322558fac
```

## Gate

`.github/workflows/webgl-stability.yml` repeats this probe on pinned Ubuntu,
Node, pnpm, Playwright, and its matching Chromium. The job has read-only
repository permission and receives no secrets because opting into software
WebGL lowers Chromium's security guarantees.

Keep the byte-exact probe as an early warning, but use the renderer-keyed
tolerance policy from the design for real conformance images because driver
upgrades and text rasterization can change pixels legitimately.

The repository had no remote during this spike, so the workflow could not yet
be observed on a GitHub-hosted runner. The first hosted pass is still required
to turn this conditional result into an unconditional Go. This external
operational check does not block compiler work that does not depend on pixel
baselines.
