---
title: Examples
permalink: /examples/
---

# Examples

Every source file under `examples/` is compiled in both debug and release mode
by CI. The examples are small contract demonstrations rather than a second test
suite:

- `triangle.rb`, `triangle.php`, and `triangle.pl` show the same Tier 1 program
  in all three adapters;
- `rotating_cubes.rb`, `.php`, and `.pl` show the same retained scene and make
  language differences easy to compare;
- `terrain.rb` constructs a custom interleaved mesh with explicit indices;
- `interactive.rb` consumes pointer and keyboard input through `on_event`;
- `texture_lifecycle.rb` packages a relative texture, joins the setup load
  barrier, and explicitly disposes it;
- `runtime_error.rb` deliberately reads a missing map key so the source-located
  browser error overlay can be inspected;
- `plasma.rb` defines a custom shader pair.

Build an example without installing the CLI:

```console
cargo run -p polygl-cli -- build examples/interactive.rb -o /tmp/polygl-example
```

Serve the result through HTTP. Opening `index.html` through `file:` is not a
supported module-loading environment.
