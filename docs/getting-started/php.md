---
title: Getting started with PHP
description: Compile and serve a PHP WebGL sketch with PolyGL.
---

# Getting started with PHP

The PHP adapter uses explicit Common Core semantics: conditions must infer as
`bool`, `/` is floating-point division, and strict equality is required. The
source is parsed and compiled; no PHP runtime or web server is involved.

## 1. Install the CLI

```console
npm install --global @polygl/cli
polygl languages
```

The language list must include `php .php`.

## 2. Create `sketch.php`

```php
<?php
function setup() {
    size(640, 360);
    background(0.03, 0.04, 0.08);
    fill(1.0, 0.35, 0.12);
    triangle(80.0, 300.0, 320.0, 55.0, 560.0, 300.0);
}
```

## 3. Check and serve

```console
polygl check sketch.php
polygl serve sketch.php --watch
```

Open <http://127.0.0.1:4173>. Use `frame(float $dt)` for per-frame work.
Conditions such as `if ($value)` must be rewritten as explicit comparisons;
PolyGL reports the source span and a suggestion when portable semantics would
otherwise be ambiguous.

Continue with
[`examples/rotating_cubes.php`](https://github.com/ydah/polygl/blob/main/examples/rotating_cubes.php)
or read the [PHP adapter reference]({{ '/adapters/php/' | relative_url }}).
