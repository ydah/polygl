import { expect, test } from "@playwright/test";
import { spawnSync } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const workspaceRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
const conformanceRoot = path.join(workspaceRoot, "conformance");
const executable = path.join(workspaceRoot, "target", "debug", "polygl");
const manifest = JSON.parse(
  await readFile(path.join(conformanceRoot, "cases.json"), "utf8"),
);
const baselineEnvironment = JSON.parse(
  await readFile(
    path.join(conformanceRoot, "l1-render", "environment.json"),
    "utf8",
  ),
);
const browserCases = manifest.filter((item) => item.browser);
const renderCases = browserCases.filter((item) =>
  item.layers.includes("l1-render"),
);
const languageFiles = {
  ruby: "main.rb",
  php: "main.php",
  perl: "main.pl",
};

let buildRoot;

test.beforeAll(async ({}, testInfo) => {
  testInfo.setTimeout(180_000);
  run("cargo", ["build", "--quiet", "-p", "polygl-cli"]);
  buildRoot = await mkdtemp(path.join(tmpdir(), "polygl-conformance-"));
  for (const item of browserCases) {
    for (const language of item.languages) {
      const file = languageFiles[language];
      run(executable, [
        "build",
        path.join(conformanceRoot, "cases", item.id, file),
        "-o",
        path.join(buildRoot, item.id, language),
        "--release",
      ]);
    }
  }
  run(executable, [
    "build",
    path.join(
      conformanceRoot,
      "semantic-cases",
      "ruby",
      "array-bounds.rb",
    ),
    "-o",
    path.join(buildRoot, "source-location", "ruby"),
    "--debug",
  ]);
  await writeFile(
    path.join(buildRoot, "rectangle", "ruby", "red.svg"),
    "<svg xmlns='http://www.w3.org/2000/svg' width='1' height='1'><rect width='1' height='1' fill='#ff0000'/></svg>",
  );
});

test.afterAll(async () => {
  if (buildRoot !== undefined) {
    await rm(buildRoot, { recursive: true, force: true });
  }
});

for (const item of renderCases) {
  for (const language of item.languages) {
    const name = item.id;
    test(`${name} in ${language} matches the SwiftShader framebuffer`, async (
      { browser, page },
      testInfo,
    ) => {
      expect(browser.version()).toBe(baselineEnvironment.browser.version);
      await page.addInitScript(() => {
        const originalGetContext = HTMLCanvasElement.prototype.getContext;
        HTMLCanvasElement.prototype.getContext = function getContext(
          contextId,
          options,
        ) {
          return originalGetContext.call(
            this,
            contextId,
            contextId === "webgl2"
              ? { ...options, preserveDrawingBuffer: true }
              : options,
          );
        };
      });
      await routeBuild(page);
      await page.goto(`http://polygl.test/${name}/${language}/index.html`);
      await page.evaluate(() => globalThis.__polyglReady);
      const frame = await page.evaluate(() => {
        const canvas = document.querySelector("canvas");
        const gl = canvas.getContext("webgl2");
        gl.finish();
        const pixels = new Uint8Array(canvas.width * canvas.height * 4);
        gl.readPixels(
          0,
          0,
          canvas.width,
          canvas.height,
          gl.RGBA,
          gl.UNSIGNED_BYTE,
          pixels,
        );
        const debugInfo = gl.getExtension("WEBGL_debug_renderer_info");
        return {
          width: canvas.width,
          height: canvas.height,
          pixels: Array.from(pixels),
          renderer:
            debugInfo === null
              ? gl.getParameter(gl.RENDERER)
              : gl.getParameter(debugInfo.UNMASKED_RENDERER_WEBGL),
        };
      });
      expect(frame.renderer).toContain("SwiftShader");
      expect(
        frame.pixels.some((value, index) => index % 4 === 3 && value !== 0),
      ).toBe(true);

      const baseline = `${frame.width}x${frame.height}\n${Buffer.from(
        frame.pixels,
      ).toString("hex")}\n`;
      const baselinePath = path.join(
        conformanceRoot,
        "l1-render",
        name,
        "swiftshader.rgba",
      );
      if (process.env.UPDATE_BASELINES === "1") {
        await mkdir(path.dirname(baselinePath), { recursive: true });
        await writeFile(baselinePath, baseline);
      } else {
        const expected = await readFile(baselinePath, "utf8");
        if (baseline !== expected) {
          const expectedLines = expected.trimEnd().split("\n");
          const [expectedWidth, expectedHeight] = expectedLines[0]
            .split("x")
            .map(Number);
          const expectedPixels = Buffer.from(expectedLines[1], "hex");
          const actualPixels = Buffer.from(frame.pixels);
          const diffPixels = Buffer.alloc(actualPixels.length);
          for (let offset = 0; offset < actualPixels.length; offset += 4) {
            const changed =
              actualPixels[offset] !== expectedPixels[offset] ||
              actualPixels[offset + 1] !== expectedPixels[offset + 1] ||
              actualPixels[offset + 2] !== expectedPixels[offset + 2] ||
              actualPixels[offset + 3] !== expectedPixels[offset + 3];
            diffPixels.set(changed ? [255, 0, 0, 255] : [0, 0, 0, 255], offset);
          }
          await testInfo.attach("expected-frame.ppm", {
            body: rgbaToPpm(expectedWidth, expectedHeight, expectedPixels),
            contentType: "image/x-portable-pixmap",
          });
          await testInfo.attach("actual-frame.ppm", {
            body: rgbaToPpm(frame.width, frame.height, actualPixels),
            contentType: "image/x-portable-pixmap",
          });
          await testInfo.attach("frame-diff.ppm", {
            body: rgbaToPpm(frame.width, frame.height, diffPixels),
            contentType: "image/x-portable-pixmap",
          });
        }
        expect(baseline).toBe(expected);
      }
    });
  }
}

const plasma = browserCases.find((item) => item.id === "plasma");
for (const language of plasma.languages) {
  test(`plasma shader in ${language} compiles and resolves its material`, async ({
    page,
  }) => {
    await routeBuild(page);
    await page.goto(`http://polygl.test/plasma/${language}/index.html`);
    await page.evaluate(() => globalThis.__polyglReady);
    const renderer = await page.evaluate(() => {
      const canvas = document.querySelector("canvas");
      const gl = canvas.getContext("webgl2");
      const debugInfo = gl.getExtension("WEBGL_debug_renderer_info");
      return debugInfo === null
        ? gl.getParameter(gl.RENDERER)
        : gl.getParameter(debugInfo.UNMASKED_RENDERER_WEBGL);
    });
    expect(renderer).toContain("SwiftShader");
  });
}

test("text overlay follows and restores a custom canvas", async ({ page }) => {
  await routeBuild(page);
  await page.goto("http://polygl.test/rectangle/ruby/index.html");
  await page.evaluate(() => globalThis.__polyglReady);
  const result = await page.evaluate(async () => {
    const runtime = await import("./runtime.js");
    const host = document.createElement("section");
    host.style.display = "flex";
    host.style.padding = "7px";
    const canvas = document.createElement("canvas");
    canvas.id = "custom-canvas";
    canvas.width = 64;
    canvas.height = 48;
    canvas.style.width = "128px";
    canvas.style.height = "96px";
    host.append(canvas);
    document.body.append(host);

    const handle = await runtime.start(
      {
        setup() {
          runtime.text("overlay", 4, 12);
        },
      },
      { canvas },
    );
    const overlay = document.getElementById("custom-canvas-text");
    const canvasBounds = canvas.getBoundingClientRect();
    const overlayBounds = overlay.getBoundingClientRect();
    const aligned = {
      height: overlayBounds.height === canvasBounds.height,
      left: overlayBounds.left === canvasBounds.left,
      top: overlayBounds.top === canvasBounds.top,
      width: overlayBounds.width === canvasBounds.width,
    };
    const wrapperTag = canvas.parentElement.tagName;
    handle.stop();
    return {
      aligned,
      restored: canvas.parentElement === host,
      wrapperTag,
    };
  });

  expect(result.aligned).toEqual({
    height: true,
    left: true,
    top: true,
    width: true,
  });
  expect(result.wrapperTag).toBe("DIV");
  expect(result.restored).toBe(true);
});

test("debug runtime overlay reports the generated source location", async ({
  page,
}) => {
  await routeBuild(page);
  await page.goto("http://polygl.test/source-location/ruby/index.html");
  const overlay = page.locator("#polygl-error-overlay");
  await expect(overlay).toBeVisible();
  await expect(overlay).toContainText(
    /array-bounds\.rb:3:14: index 1 is outside 0\.\.0/,
  );
});

test("browser resize, DPR, and context loss follow the session contract", async ({
  page,
}) => {
  await routeBuild(page);
  await page.goto("http://polygl.test/rectangle/ruby/index.html");
  await page.evaluate(() => globalThis.__polyglReady);
  const result = await page.evaluate(async () => {
    const runtime = await import("./runtime.js");
    const canvas = document.createElement("canvas");
    canvas.style.width = "80px";
    canvas.style.height = "40px";
    document.body.append(canvas);
    const errors = [];
    const handle = await runtime.start(
      { frame() {} },
      {
        autoResize: true,
        canvas,
        devicePixelRatio: 2,
        onError(reason) {
          errors.push(String(reason));
        },
      },
    );
    const initial = [canvas.width, canvas.height];
    canvas.style.width = "50px";
    canvas.style.height = "30px";
    await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    const resized = [canvas.width, canvas.height];
    const extension = canvas
      .getContext("webgl2")
      .getExtension("WEBGL_lose_context");
    if (extension === null) {
      handle.stop();
      canvas.remove();
      return { contextLossSupported: false, initial, resized };
    }
    extension.loseContext();
    for (let attempt = 0; attempt < 20 && errors.length === 0; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 5));
    }
    const stateAfterLoss = handle.state;
    handle.stop();
    canvas.remove();
    return {
      contextLossSupported: true,
      errors,
      initial,
      resized,
      stateAfterLoss,
    };
  });
  expect(result.contextLossSupported).toBe(true);
  expect(result.initial).toEqual([160, 80]);
  expect(result.resized).toEqual([100, 60]);
  expect(result.stateAfterLoss).toBe("context-lost");
  expect(result.errors[0]).toContain("rendering is suspended");
});

test("a loaded texture is sampled into a real framebuffer", async ({ page }) => {
  await page.addInitScript(() => {
    const originalGetContext = HTMLCanvasElement.prototype.getContext;
    HTMLCanvasElement.prototype.getContext = function getContext(
      contextId,
      options,
    ) {
      return originalGetContext.call(
        this,
        contextId,
        contextId === "webgl2"
          ? { ...options, preserveDrawingBuffer: true }
          : options,
      );
    };
  });
  await routeBuild(page);
  await page.goto("http://polygl.test/rectangle/ruby/index.html");
  await page.evaluate(() => globalThis.__polyglReady);
  const pixel = await page.evaluate(async () => {
    const runtime = await import("./runtime.js");
    const location = {
      source: "texture-framebuffer.test",
      line: 1,
      column: 1,
      start: 0,
      end: 1,
    };
    const shader = {
      name: "texture_framebuffer",
      vertex: `#version 300 es
precision highp float;
layout(location = 0) in vec3 a_position;
layout(location = 2) in vec2 a_uv;
out vec2 v_uv;
void main() {
  v_uv = a_uv;
  gl_Position = vec4(a_position.x, a_position.z, 0.0, 1.0);
}`,
      fragment: `#version 300 es
precision highp float;
in vec2 v_uv;
uniform sampler2D pgl_u_texture_map;
out vec4 out_color;
void main() { out_color = texture(pgl_u_texture_map, v_uv); }`,
      attributes: [
        {
          name: "position",
          glslName: "a_position",
          location: 0,
          type: "vec3",
        },
        { name: "uv", glslName: "a_uv", location: 2, type: "vec2" },
      ],
      uniforms: [
        {
          name: "texture_map",
          glslName: "pgl_u_texture_map",
          type: "texture",
          source: "user",
        },
      ],
      vertexLocation: location,
      fragmentLocation: location,
    };
    const canvas = document.createElement("canvas");
    canvas.width = 4;
    canvas.height = 4;
    document.body.append(canvas);
    const handle = await runtime.start(
      {
        __polyglShaderBundle: {
          debug: true,
          shaderAbi: runtime.shaderAbi,
          shaders: [shader],
        },
        setup() {
          runtime.size(4, 4);
          const texture = runtime.textureLoad("red.svg");
          const node = runtime.nodeAdd(
            runtime.meshPlane(2, 2, 1, 1),
            runtime.materialShader("texture_framebuffer"),
          );
          runtime.shaderSet(node, "texture_map", texture);
        },
      },
      { canvas },
    );
    const gl = canvas.getContext("webgl2");
    gl.finish();
    const value = new Uint8Array(4);
    gl.readPixels(2, 2, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, value);
    handle.stop();
    return Array.from(value);
  });
  expect(pixel).toEqual([255, 0, 0, 255]);
});

test("unset user uniforms fail in debug and retain defaults in release", async ({
  page,
}) => {
  await routeBuild(page);
  await page.goto("http://polygl.test/rectangle/ruby/index.html");
  await page.evaluate(() => globalThis.__polyglReady);
  const result = await page.evaluate(async () => {
    const runtime = await import("./runtime.js");
    const location = {
      source: "uniform-policy.test",
      line: 7,
      column: 4,
      start: 10,
      end: 20,
    };
    const shader = {
      name: "uniform_policy",
      vertex: `#version 300 es
precision highp float;
layout(location = 0) in vec3 a_position;
void main() { gl_Position = vec4(a_position.x, a_position.z, 0.0, 1.0); }`,
      fragment: `#version 300 es
precision highp float;
uniform vec4 pgl_u_tint;
out vec4 out_color;
void main() { out_color = pgl_u_tint; }`,
      attributes: [
        {
          name: "position",
          glslName: "a_position",
          location: 0,
          type: "vec3",
        },
      ],
      uniforms: [
        {
          name: "tint",
          glslName: "pgl_u_tint",
          type: "vec4",
          source: "user",
        },
      ],
      vertexLocation: location,
      fragmentLocation: location,
    };
    const source = {
      setup() {
        runtime.nodeAdd(
          runtime.meshPlane(2, 2, 1, 1),
          runtime.materialShader("uniform_policy"),
        );
      },
    };
    async function attempt(debug) {
      const canvas = document.createElement("canvas");
      canvas.width = 4;
      canvas.height = 4;
      document.body.append(canvas);
      try {
        const handle = await runtime.start(
          {
            ...source,
            __polyglShaderBundle: {
              debug,
              shaderAbi: runtime.shaderAbi,
              shaders: [shader],
            },
          },
          { canvas, onError() {} },
        );
        handle.stop();
        return "success";
      } catch (error) {
        return runtime.formatRuntimeError(error);
      } finally {
        canvas.remove();
      }
    }
    return { debug: await attempt(true), release: await attempt(false) };
  });
  expect(result.debug).toContain(
    "uniform-policy.test:7:4: user uniform `tint` is unset",
  );
  expect(result.release).toBe("success");
});

async function routeBuild(page) {
  await page.route("http://polygl.test/**", async (route) => {
    const url = new URL(route.request().url());
    const file = path.resolve(buildRoot, `.${decodeURIComponent(url.pathname)}`);
    if (!file.startsWith(`${path.resolve(buildRoot)}${path.sep}`)) {
      await route.fulfill({ status: 403, body: "forbidden" });
      return;
    }
    try {
      await route.fulfill({
        status: 200,
        body: await readFile(file),
        contentType: contentType(file),
      });
    } catch {
      await route.fulfill({ status: 404, body: "not found" });
    }
  });
}

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: workspaceRoot,
    encoding: "utf8",
  });
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} failed\n${result.stdout}\n${result.stderr}`,
    );
  }
}

function contentType(file) {
  switch (path.extname(file)) {
    case ".html":
      return "text/html; charset=utf-8";
    case ".js":
      return "text/javascript; charset=utf-8";
    case ".map":
      return "application/json";
    case ".svg":
      return "image/svg+xml";
    default:
      return "application/octet-stream";
  }
}

function rgbaToPpm(width, height, rgba) {
  const rgb = Buffer.alloc(width * height * 3);
  for (let source = 0, target = 0; source < rgba.length; source += 4) {
    rgb[target++] = rgba[source];
    rgb[target++] = rgba[source + 1];
    rgb[target++] = rgba[source + 2];
  }
  return Buffer.concat([Buffer.from(`P6\n${width} ${height}\n255\n`), rgb]);
}
