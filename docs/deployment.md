---
title: Static deployment
permalink: /deployment/
---

# Static deployment

Build release output with hashed entrypoints for long-lived static hosting:

```console
polygl build --config polygl.toml --release --hashed-filenames
```

Set `base_url` to an absolute path beginning and ending with `/`, for example
`/art/sketch/`. Protocol-relative and full external URLs are rejected. Custom
templates must contain the one documented marker; public trees and compiled
assets must remain inside their canonical roots and cannot contain symlinks or
portable path collisions.

## Caching and MIME

- Cache content-hashed `.js` and `.map` entrypoints and immutable assets with a
  long lifetime and `immutable`.
- Serve `index.html` and `polygl-manifest.json` with revalidation or a short
  lifetime so a new generation becomes discoverable.
- Serve JavaScript as `text/javascript`, maps/manifests as `application/json`,
  SVG as `image/svg+xml`, and common raster/font assets with their registered
  media type. Do not rely on MIME sniffing.
- Deploy the entire generated directory as one generation. Do not copy files
  over an older live directory one at a time.

The manifest's BLAKE3 values are deployment integrity metadata, not browser SRI
attributes. Verify them in the publishing pipeline if artifacts cross an
untrusted store.

## Content Security Policy

Generated output uses local ES modules and does not need `eval`. Start with:

```text
default-src 'none'; script-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; connect-src 'self'
```

The generated error/text overlays currently use inline style properties, hence
the style exception. Extend `img-src` or `connect-src` only for intentional
application resources. Host untrusted generated programs on a separate origin;
the runtime is not a JavaScript sandbox.

## Source Maps and privacy

Release mode defaults to no Source Map. External or inline maps can reveal
normalized source names, control flow, comments/literals, and—with
`sources_content`—the complete input. Restrict or omit maps in public hosting;
do not assume an unlinked `.map` is secret. Hashed map names reduce accidental
guessing but are not access control.
