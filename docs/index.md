---
title: Documentation
description: Compile Ruby, PHP, and Perl graphics programs to JavaScript and GLSL for WebGL 2.
permalink: /
---

<section class="hero">
  <div>
    <p class="eyebrow">Three languages. One graphics pipeline.</p>
    <h1>Sketch outside the box.</h1>
    <p class="hero-copy">
      PolyGL turns a focused Ruby, PHP, or Perl program into typed JavaScript,
      GLSL ES 3.00, and a batched WebGL 2 application.
    </p>
    <div class="hero-actions">
      <a class="button" href="{{ '/getting-started/ruby/' | relative_url }}">Start with Ruby</a>
      <a class="button secondary" href="{{ '/cli/' | relative_url }}">Explore the CLI</a>
    </div>
  </div>
  <div class="terminal" aria-label="PolyGL terminal example">
    <div class="terminal-bar" aria-hidden="true"><i></i><i></i><i></i></div>
    <pre><span class="prompt">$</span> npm i -g @polygl/cli
<span class="prompt">$</span> polygl languages
ruby    .rb
php     .php
perl    .pl
<span class="prompt">$</span> polygl serve sketch.rb --watch
http://127.0.0.1:4173</pre>
  </div>
</section>

<section class="topic-section">
  <div class="section-heading">
    <h2>Find your entry point.</h2>
    <p>Choose a source language or go directly to the compiler and runtime contracts.</p>
  </div>
  <div class="topic-grid" data-search-grid>
    <a class="topic-card" data-search-card href="{{ '/getting-started/ruby/' | relative_url }}">
      <span class="topic-number">01 / LANGUAGE</span>
      <h3>Ruby</h3>
      <p>Use familiar methods, blocks, arrays, maps, and struct-like classes.</p>
    </a>
    <a class="topic-card" data-search-card href="{{ '/getting-started/php/' | relative_url }}">
      <span class="topic-number">02 / LANGUAGE</span>
      <h3>PHP</h3>
      <p>Compile strict typed functions and explicit boolean graphics logic.</p>
    </a>
    <a class="topic-card" data-search-card href="{{ '/getting-started/perl/' | relative_url }}">
      <span class="topic-number">03 / LANGUAGE</span>
      <h3>Perl</h3>
      <p>Lower a practical, static Perl 5 subset through the same pipeline.</p>
    </a>
    <a class="topic-card" data-search-card href="{{ '/api/' | relative_url }}">
      <span class="topic-number">04 / REFERENCE</span>
      <h3>Graphics API</h3>
      <p>Browse Tier 1 drawing, input, shader, and retained Tier 2 operations.</p>
    </a>
    <a class="topic-card" data-search-card href="{{ '/common-core/' | relative_url }}">
      <span class="topic-number">05 / COMPILER</span>
      <h3>Common Core</h3>
      <p>Understand the portable language subset and its fixed semantics.</p>
    </a>
    <a class="topic-card" data-search-card href="{{ '/adapter-guide/' | relative_url }}">
      <span class="topic-number">06 / EXTEND</span>
      <h3>Add a language</h3>
      <p>Build an adapter with stable spans, diagnostics, and conformance gates.</p>
    </a>
    <a class="topic-card" data-search-card href="{{ '/shader-abi/' | relative_url }}">
      <span class="topic-number">07 / GPU</span>
      <h3>Shader ABI</h3>
      <p>Write GPU functions against the reflected GLSL ES 3.00 contract.</p>
    </a>
    <a class="topic-card" data-search-card href="{{ '/errors/' | relative_url }}">
      <span class="topic-number">08 / REFERENCE</span>
      <h3>Diagnostics</h3>
      <p>Look up stable error codes, source spans, and rewrite suggestions.</p>
    </a>
    <a class="topic-card" data-search-card href="{{ '/performance/' | relative_url }}">
      <span class="topic-number">09 / RUNTIME</span>
      <h3>Performance</h3>
      <p>Separate compiler speed from generated JavaScript and rendering cost.</p>
    </a>
  </div>
  <p class="search-status" data-search-status aria-live="polite">
    Browse by language or compiler topic
  </p>
</section>
