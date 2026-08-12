---
title: Platform and browser support
permalink: /support/
---

# Platform and browser support

| Surface | Supported contract | CI evidence |
| --- | --- | --- |
| Rust source build | Rust 1.96.1 (MSRV), stable, and beta checks | Linux; workspace tests also run macOS and Windows |
| npm launcher | Node.js 20 or newer | Node 20.0.0 and pinned development Node 24.14.1 |
| Native CLI | Linux x64/arm64, macOS x64/arm64, Windows x64 | Build, archive/hash verification, clean install, example build |
| Linux runtime floor | glibc 2.39 or older requirement | ELF version inspection on release artifacts |
| macOS runtime floor | macOS 11.0 | deployment target plus Mach-O minimum-version inspection |
| Browser baseline | Chromium 143 + pinned SwiftShader, WebGL 2 | blocking pixel/semantic conformance |
| Browser portability | current pinned Playwright Firefox and WebKit, WebGL 2 | scheduled/manual non-blocking smoke |

Browser conformance fixes the renderer, browser revision, viewport, and ABI in
`conformance/l1-render/environment.json`. Firefox and WebKit are portability
signals because different rasterization and driver stacks cannot honestly share
a byte-identical framebuffer baseline.

PolyGL requires ES modules, promises, Canvas2D for text, and WebGL 2. A browser
without WebGL 2 fails startup; there is no WebGL 1 fallback. Mobile browsers may
work when these facilities and resource limits are available, but are not in
the release matrix.

No current CI job is evidence for a physical GPU: hosted tests use software
rendering. A real-GPU guarantee requires a trusted dedicated runner, recorded
device/driver versions, tolerant baselines, and an upgrade owner.
