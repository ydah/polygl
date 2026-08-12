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

test.beforeAll(async () => {
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
});

test.afterAll(async () => {
  if (buildRoot !== undefined) {
    await rm(buildRoot, { recursive: true, force: true });
  }
});

for (const item of renderCases) {
  for (const language of item.languages) {
    const name = item.id;
    test(`${name} in ${language} matches the SwiftShader framebuffer`, async ({
      page,
    }) => {
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
        expect(baseline).toBe(await readFile(baselinePath, "utf8"));
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
    default:
      return "application/octet-stream";
  }
}
