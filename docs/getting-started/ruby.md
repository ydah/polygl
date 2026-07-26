---
title: Getting started with Ruby
description: Compile and serve a Ruby WebGL sketch with PolyGL.
---

# Getting started with Ruby

Ruby is the broadest PolyGL adapter. It supports the Common Core, Tier 1 and
Tier 2 graphics, shaders, arrays, string-keyed hashes, whitelisted
`times`/`each` blocks, and struct-like classes.

## 1. Install the CLI

Install a tagged native build through npm:

```console
npm install --global @polygl/cli
polygl languages
```

The language list must include `ruby .rb`. See the
[CLI reference]({{ '/cli/' | relative_url }}) for release archives and source
installation.

## 2. Create `sketch.rb`

```ruby
def setup
  size(640, 360)
  background(0.03, 0.04, 0.08)
  fill(0.2, 0.75, 1.0)
  triangle(80.0, 300.0, 320.0, 55.0, 560.0, 300.0)
end
```

PolyGL compiles a supported Ruby subset; it does not start a Ruby interpreter.
Graphics functions resolve through the shared typed API.

## 3. Check and serve

```console
polygl check sketch.rb
polygl serve sketch.rb --watch
```

Open <http://127.0.0.1:4173>. Saving the file rebuilds and reloads the page. A
failed rebuild keeps the last valid sketch running and shows the diagnostic in
the browser.

For a 3D next step, compile
[`examples/rotating_cubes.rb`](https://github.com/ydah/polygl/blob/main/examples/rotating_cubes.rb).
The complete syntax mapping is in the
[Ruby adapter reference]({{ '/adapters/ruby/' | relative_url }}).
