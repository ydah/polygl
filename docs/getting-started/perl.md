---
title: Getting started with Perl
description: Compile and serve a Perl 5 WebGL sketch with PolyGL.
---

# Getting started with Perl

The Perl adapter targets a deliberately static Perl 5 subset. It accepts
functions, lexical variables, arrays, maps, packages in the struct-like class
subset, Tier 1 and Tier 2 graphics, and shaders. It does not execute Perl or
load CPAN modules.

## 1. Install the CLI

```console
npm install --global @polygl/cli
polygl languages
```

The language list must include `perl .pl`.

## 2. Create `sketch.pl`

```perl
use strict;
use warnings;

sub setup {
    size(640, 360);
    background(0.03, 0.04, 0.08);
    fill(0.75, 0.35, 1.0);
    triangle(80.0, 300.0, 320.0, 55.0, 560.0, 300.0);
}
```

## 3. Check and serve

```console
polygl check sketch.pl
polygl serve sketch.pl --watch
```

Open <http://127.0.0.1:4173>. A frame function receives its parameters through
the supported leading `@_` destructuring form:

```perl
sub frame {
    my ($dt) = @_;
    background(0.03, 0.04, 0.08);
}
```

Continue with
[`examples/rotating_cubes.pl`](https://github.com/ydah/polygl/blob/main/examples/rotating_cubes.pl)
or read the full [Perl adapter reference]({{ '/adapters/perl/' | relative_url }}).
