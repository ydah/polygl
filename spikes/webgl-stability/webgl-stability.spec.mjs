import { chromium, expect, test } from "@playwright/test";
import { createHash } from "node:crypto";
import {
  swiftShaderLaunchOptions,
  viewport,
} from "./swiftshader.mjs";

const pageSource = `
<!doctype html>
<html>
  <body style="margin:0;background:#000">
    <canvas id="canvas" width="320" height="240"></canvas>
    <script>
      const canvas = document.querySelector("#canvas");
      const gl = canvas.getContext("webgl2", {
        antialias: false,
        depth: false,
        preserveDrawingBuffer: true,
      });
      if (!gl) throw new Error("WebGL2 is unavailable");
      const rendererInfo = gl.getExtension("WEBGL_debug_renderer_info");
      document.documentElement.dataset.renderer = rendererInfo
        ? gl.getParameter(rendererInfo.UNMASKED_RENDERER_WEBGL)
        : gl.getParameter(gl.RENDERER);

      const compile = (type, source) => {
        const shader = gl.createShader(type);
        gl.shaderSource(shader, source);
        gl.compileShader(shader);
        if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
          throw new Error(gl.getShaderInfoLog(shader));
        }
        return shader;
      };

      const program = gl.createProgram();
      gl.attachShader(program, compile(gl.VERTEX_SHADER, \`#version 300 es
        in vec2 position;
        void main() { gl_Position = vec4(position, 0.0, 1.0); }
      \`));
      gl.attachShader(program, compile(gl.FRAGMENT_SHADER, \`#version 300 es
        precision highp float;
        out vec4 color;
        void main() { color = vec4(0.125, 0.5, 0.875, 1.0); }
      \`));
      gl.linkProgram(program);
      if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
        throw new Error(gl.getProgramInfoLog(program));
      }

      gl.useProgram(program);
      gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
      gl.bufferData(
        gl.ARRAY_BUFFER,
        new Float32Array([-0.8, -0.7, 0.8, -0.7, 0.0, 0.8]),
        gl.STATIC_DRAW,
      );
      const position = gl.getAttribLocation(program, "position");
      gl.enableVertexAttribArray(position);
      gl.vertexAttribPointer(position, 2, gl.FLOAT, false, 0, 0);
      gl.clearColor(0.0625, 0.125, 0.25, 1.0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
      gl.finish();
      document.documentElement.dataset.ready = "true";
    </script>
  </body>
</html>
`;

const digest = (buffer) => createHash("sha256").update(buffer).digest("hex");

test("SwiftShader produces byte-identical screenshots for one input", async () => {
  const screenshots = [];
  const renderers = [];

  for (let run = 0; run < 3; run += 1) {
    const browser = await chromium.launch(swiftShaderLaunchOptions);
    try {
      const page = await browser.newPage({ viewport });
      const pageErrors = [];
      page.on("pageerror", (error) => pageErrors.push(error.message));
      await page.setContent(pageSource);
      const ready = await page.locator("html").getAttribute("data-ready");
      expect(ready, `page errors: ${pageErrors.join("; ")}`).toBe("true");
      renderers.push(await page.locator("html").getAttribute("data-renderer"));
      screenshots.push(await page.locator("#canvas").screenshot());
    } finally {
      await browser.close();
    }
  }

  for (const renderer of renderers) {
    expect(renderer).toMatch(/SwiftShader/i);
  }
  const hashes = screenshots.map(digest);
  expect(new Set(hashes).size, `screenshot hashes: ${hashes.join(", ")}`).toBe(1);
  console.log(`renderer=${renderers[0]}`);
  console.log(`sha256=${hashes[0]}`);
});
