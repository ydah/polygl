import { createRequire } from "node:module";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const requireFromBrowser = createRequire(
  new URL("../../conformance/browser/package.json", import.meta.url),
);
const { chromium } = requireFromBrowser("@playwright/test");

const runtime = await readFile(
  resolve(root, "crates/polygl-cli/assets/runtime.js"),
);
const server = createServer((request, response) => {
  if (request.url === "/runtime.js") {
    response.writeHead(200, { "content-type": "text/javascript; charset=utf-8" });
    response.end(runtime);
    return;
  }
  response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
  response.end("<!doctype html><canvas id=canvas width=640 height=480></canvas>");
});
await new Promise((resolveListen, reject) => {
  server.once("error", reject);
  server.listen(0, "127.0.0.1", resolveListen);
});

const address = server.address();
if (address === null || typeof address === "string") {
  throw new Error("runtime benchmark did not bind a TCP port");
}
const browser = await chromium.launch({
  headless: true,
  args: ["--use-angle=swiftshader"],
});
try {
  const page = await browser.newPage();
  await page.goto(`http://127.0.0.1:${String(address.port)}/`);
  const result = await page.evaluate(async () => {
    const api = await import("/runtime.js");
    const canvas = document.querySelector("canvas");
    if (!(canvas instanceof HTMLCanvasElement)) throw new Error("missing canvas");

    async function runCase(program, options = {}) {
      const frames = [];
      const started = performance.now();
      const handle = await api.start(program, {
        canvas,
        onError(reason) { throw reason; },
        requestAnimationFrame(callback) {
          frames.push(callback);
          return frames.length;
        },
        cancelAnimationFrame() {},
        ...options,
      });
      const setupMs = performance.now() - started;
      const frameStarted = performance.now();
      frames.shift()?.(1_000);
      const frameMs = performance.now() - frameStarted;
      const stats = handle.stats();
      handle.stop();
      return { frameMs, setupMs, stats };
    }

    const shapes = await runCase({
      frame() {
        for (let index = 0; index < 10_000; index += 1) {
          api.rect(index % 200, Math.floor(index / 200), 1, 1);
        }
      },
    });
    const nodes = await runCase({
      setup() {
        const mesh = api.meshBox(0.05, 0.05, 0.05);
        const material = api.materialBasic([0.2, 0.7, 1, 1]);
        for (let index = 0; index < 256; index += 1) {
          const node = api.nodeAdd(mesh, material);
          api.nodeSetPos(node, (index % 16 - 8) * 0.08, (Math.floor(index / 16) - 8) * 0.08, 0);
        }
      },
    });
    const textures = await runCase({
      setup() {
        for (let index = 0; index < 32; index += 1) {
          api.textureLoad(`assets/texture-${String(index)}.png`);
        }
      },
    }, {
      imageLoader: async () => new ImageData(1, 1),
    });
    const uniforms = await runCase({}, {
      shaderBundle: {
        debug: false,
        shaders: [{
          name: "automatic",
          vertex: "#version 300 es\nuniform float u_time;\nvoid main(){gl_Position=vec4(sin(u_time)*0.001,0.0,0.0,1.0);}",
          fragment: "#version 300 es\nprecision highp float;\nout vec4 color;\nvoid main(){color=vec4(1.0);}",
          attributes: [],
          uniforms: [{ name: "u_time", glslName: "u_time", type: "float", source: "automatic" }],
          vertexLocation: { source: "benchmark", line: 1, column: 1, start: 0, end: 1 },
          fragmentLocation: { source: "benchmark", line: 1, column: 1, start: 0, end: 1 },
        }],
      },
    });
    const gl = canvas.getContext("webgl2");
    return {
      cases: { nodes, shapes, textures, uniforms },
      environment: {
        renderer: gl?.getParameter(gl.RENDERER),
        userAgent: navigator.userAgent,
        vendor: gl?.getParameter(gl.VENDOR),
        webglVersion: gl?.getParameter(gl.VERSION),
      },
    };
  });
  process.stdout.write(`${JSON.stringify({ schemaVersion: 1, ...result }, null, 2)}\n`);
} finally {
  await browser.close();
  await new Promise((resolveClose, reject) => {
    server.close((error) => error === undefined ? resolveClose() : reject(error));
  });
}
