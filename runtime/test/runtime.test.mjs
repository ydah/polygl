import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const runtimeBundle = await readFile(
  new URL("../../crates/polygl-cli/assets/runtime.js", import.meta.url),
);
const {
  SeededRandom,
  checkedIndex,
  circle,
  fill,
  formatRuntimeError,
  materialShader,
  random,
  rect,
  roundToInt,
  runtimeOps,
  runtimeVersion,
  setShaderUniform,
  size,
  start,
  time,
  triangle,
  width,
} = await import(`data:text/javascript;base64,${runtimeBundle.toString("base64")}`);

test("exports generated runtime metadata", () => {
  assert.equal(runtimeOps.background, "background");
  assert.equal(runtimeOps.no_stroke, "noStroke");
  assert.equal(runtimeOps.time, "time");
  assert.equal(runtimeOps.material_shader, "materialShader");
  assert.equal(runtimeVersion, "0.0.0");
});

test("seeded random sequences are reproducible", () => {
  const first = new SeededRandom(42);
  const second = new SeededRandom(42);
  assert.deepEqual(
    [first.next(), first.next(), first.between(-2, 3)],
    [second.next(), second.next(), second.between(-2, 3)],
  );
});

test("debug checks preserve original source locations", () => {
  const location = {
    source: "main.rb",
    line: 12,
    column: 4,
    start: 100,
    end: 109,
  };
  let failure;
  try {
    checkedIndex([1, 2], 3, location);
  } catch (error) {
    failure = error;
  }
  assert.ok(failure instanceof Error);
  assert.equal(
    formatRuntimeError(failure),
    "main.rb:12:4: index 3 is outside 0..1",
  );
  assert.equal(roundToInt(-1.5), -2);
  assert.equal(roundToInt(1.5), 2);
});

test("runs setup and frames while batching shape vertices", async () => {
  const { context, draws, clears } = fakeWebGl2();
  const canvas = fakeCanvas();
  const frames = [];
  const cancelled = [];
  const frameDeltas = [];
  const frameTimes = [];
  const failures = [];
  const loadValues = [];

  const handle = await start(
    async () => {
      loadValues.push(width(), random(0, 1));
      return {
        setup() {
          size(320, 180);
          fill(0.25, 0.5, 0.75);
          rect(0, 0, 10, 20);
          circle(20, 20, 5);
        },
        frame(dt) {
          frameDeltas.push(dt);
          frameTimes.push(time());
          triangle(0, 0, 1, 0, 0, 1);
        },
      };
    },
    {
      canvas,
      context,
      requestAnimationFrame(callback) {
        frames.push(callback);
        return frames.length;
      },
      cancelAnimationFrame(handleToCancel) {
        cancelled.push(handleToCancel);
      },
      onError(reason) {
        failures.push(reason);
      },
      seed: 7,
    },
  );

  assert.equal(canvas.width, 320);
  assert.equal(canvas.height, 180);
  assert.equal(loadValues[0], 64);
  assert.ok(loadValues[1] >= 0 && loadValues[1] < 1);
  assert.deepEqual(draws, [102]);
  assert.equal(clears.length, 0);
  assert.equal(frames.length, 1);

  frames[0](1_000);
  assert.deepEqual(frameDeltas, [0]);
  assert.deepEqual(frameTimes, [0]);
  assert.deepEqual(draws, [102, 3]);
  assert.equal(frames.length, 2);
  assert.deepEqual(failures, []);

  frames[1](1_016);
  assert.deepEqual(frameDeltas, [0, 0.016]);
  assert.ok(Math.abs(frameTimes[1] - 0.016) < Number.EPSILON);

  handle.stop();
  assert.deepEqual(cancelled, [3]);
  assert.equal(canvas.listeners.size, 0);
  assert.throws(() => fill(1, 1, 1), /has not been started/);
});

test("rejects overlapping asynchronous starts", async () => {
  const { context } = fakeWebGl2();
  const canvas = fakeCanvas();
  let enterSetup;
  let finishSetup;
  const setupEntered = new Promise((resolve) => {
    enterSetup = resolve;
  });
  const setupBlocked = new Promise((resolve) => {
    finishSetup = resolve;
  });

  const first = start(
    {
      async setup() {
        enterSetup();
        await setupBlocked;
      },
    },
    { canvas, context, onError() {} },
  );
  await setupEntered;

  await assert.rejects(
    start({}, { canvas, context, onError() {} }),
    /already starting/,
  );
  finishSetup();
  const handle = await first;
  handle.stop();
});

test("rejects failed startup and clears the active session", async () => {
  const { context } = fakeWebGl2();
  const canvas = fakeCanvas();
  const failures = [];
  const startupError = new Error("setup failed");

  await assert.rejects(
    start(
      {
        setup() {
          throw startupError;
        },
      },
      {
        canvas,
        context,
        onError(reason) {
          failures.push(reason);
        },
      },
    ),
    startupError,
  );
  assert.deepEqual(failures, [startupError]);
  assert.throws(() => fill(1, 1, 1), /has not been started/);
  assert.equal(canvas.listeners.size, 0);
});

test("compiles reflected shaders and uploads automatic and user uniforms", async () => {
  const {
    context,
    uniform1fValues,
    uniform3fvValues,
  } = fakeWebGl2();
  const canvas = fakeCanvas();
  const frames = [];
  let material;
  const bundle = shaderBundle([
    {
      name: "plasma",
      uniforms: [
        {
          name: "u_time",
          glslName: "u_time",
          type: "float",
          source: "automatic",
        },
        {
          name: "tint",
          glslName: "pgl_u_tint",
          type: "vec3",
          source: "user",
        },
      ],
    },
  ]);

  const handle = await start(
    {
      __polyglShaderBundle: bundle,
      setup() {
        material = materialShader("plasma");
        const tint = [0.2, 0.4, 0.6];
        setShaderUniform("plasma", "tint", tint);
        tint[0] = Number.NaN;
        tint.length = 0;
      },
      frame() {},
    },
    {
      canvas,
      context,
      requestAnimationFrame(callback) {
        frames.push(callback);
        return frames.length;
      },
      cancelAnimationFrame() {},
      onError() {},
    },
  );

  assert.deepEqual(uniform1fValues, [0]);
  assert.deepEqual(uniform3fvValues, [[0.2, 0.4, 0.6]]);
  assert.deepEqual(material, { kind: "shader", shaderName: "plasma" });
  assert.equal(Object.isFrozen(material), true);
  assert.strictEqual(materialShader("plasma"), material);
  assert.throws(() => materialShader("missing"), /unknown shader pair `missing`/);
  frames[0](1_000);
  frames[1](1_016);
  assert.deepEqual(uniform1fValues, [0, 0, 0.016]);
  handle.stop();
});

test("allows optimized-out uniforms and rejects invalid uploads", async () => {
  const inactive = fakeWebGl2({ inactiveUniform: "u_time" });
  const handle = await start(
    {},
    {
      canvas: fakeCanvas(),
      context: inactive.context,
      shaderBundle: shaderBundle([
        {
          name: "optimized",
          uniforms: [
            {
              name: "u_time",
              glslName: "u_time",
              type: "float",
              source: "automatic",
            },
          ],
        },
      ]),
      onError() {},
    },
  );
  assert.deepEqual(inactive.uniform1fValues, []);
  handle.stop();

  const integer = fakeWebGl2();
  await assert.rejects(
    start(
      {
        __polyglShaderBundle: shaderBundle([
          {
            name: "integer",
            uniforms: [
              {
                name: "count",
                glslName: "pgl_u_count",
                type: "int",
                source: "user",
              },
            ],
          },
        ]),
        setup() {
          setShaderUniform("integer", "count", 2_147_483_648);
        },
      },
      {
        canvas: fakeCanvas(),
        context: integer.context,
        onError() {},
      },
    ),
    /uniform `count` expects int/,
  );
});

test("releases built-in renderer resources when startup fails", async () => {
  const failed = fakeWebGl2({ failShaderAt: 2 });
  await assert.rejects(
    start(
      {},
      {
        canvas: fakeCanvas(),
        context: failed.context,
        onError() {},
      },
    ),
    /built-in WebGL2 shader/,
  );
  assert.equal(failed.deletedShaders.length, 2);
});

test("reports shader startup failures at the originating source", async () => {
  const missing = fakeWebGl2();
  await assert.rejects(
    start(
      {
        __polyglShaderBundle: shaderBundle([
          {
            name: "needs_color",
            uniforms: [
              {
                name: "color",
                glslName: "pgl_u_color",
                type: "vec4",
                source: "user",
              },
            ],
          },
        ]),
      },
      {
        canvas: fakeCanvas(),
        context: missing.context,
        onError() {},
      },
    ),
    (error) => {
      assert.equal(
        formatRuntimeError(error),
        "main.rb:8:3: user uniform `color` is unset for shader `needs_color`",
      );
      return true;
    },
  );

  const failedCompile = fakeWebGl2({ failShaderAt: 3 });
  await assert.rejects(
    start(
      {
        __polyglShaderBundle: shaderBundle([{ name: "broken" }]),
      },
      {
        canvas: fakeCanvas(),
        context: failedCompile.context,
        onError() {},
      },
    ),
    (error) => {
      assert.match(
        formatRuntimeError(error),
        /main\.rb:4:2: failed to compile vertex shader `broken`: driver rejected source/,
      );
      return true;
    },
  );
});

function shaderBundle(shaders) {
  return {
    debug: true,
    shaders: shaders.map((shader) => ({
      vertex: "#version 300 es\nvoid main() {}",
      fragment: "#version 300 es\nvoid main() {}",
      attributes: [],
      uniforms: [],
      vertexLocation: {
        source: "main.rb",
        line: 4,
        column: 2,
        start: 20,
        end: 40,
      },
      fragmentLocation: {
        source: "main.rb",
        line: 8,
        column: 3,
        start: 60,
        end: 80,
      },
      ...shader,
    })),
  };
}

function fakeCanvas() {
  const listeners = new Map();
  return {
    width: 64,
    height: 64,
    listeners,
    addEventListener(name, listener) {
      listeners.set(name, listener);
    },
    removeEventListener(name, listener) {
      if (listeners.get(name) === listener) {
        listeners.delete(name);
      }
    },
    getBoundingClientRect() {
      return { left: 0, top: 0, width: this.width, height: this.height };
    },
  };
}

function fakeWebGl2(options = {}) {
  const draws = [];
  const clears = [];
  const uniform1fValues = [];
  const uniform3fvValues = [];
  const deletedShaders = [];
  let shaderChecks = 0;
  const context = {
    ARRAY_BUFFER: 0x8892,
    BLEND: 0x0be2,
    COLOR_BUFFER_BIT: 0x4000,
    COMPILE_STATUS: 0x8b81,
    DYNAMIC_DRAW: 0x88e8,
    FLOAT: 0x1406,
    FRAGMENT_SHADER: 0x8b30,
    LINK_STATUS: 0x8b82,
    NO_ERROR: 0,
    ONE_MINUS_SRC_ALPHA: 0x0303,
    SRC_ALPHA: 0x0302,
    TRIANGLES: 0x0004,
    TEXTURE0: 0x84c0,
    TEXTURE_2D: 0x0de1,
    VERTEX_SHADER: 0x8b31,
    activeTexture() {},
    attachShader() {},
    bindBuffer() {},
    bindTexture() {},
    blendFunc() {},
    bufferData() {},
    clear(mask) {
      clears.push(mask);
    },
    clearColor() {},
    compileShader() {},
    createBuffer() {
      return {};
    },
    createProgram() {
      return {};
    },
    createShader() {
      return {};
    },
    deleteBuffer() {},
    deleteProgram() {},
    deleteShader(shader) {
      deletedShaders.push(shader);
    },
    drawArrays(_mode, _first, count) {
      draws.push(count);
    },
    enable() {},
    enableVertexAttribArray() {},
    getAttribLocation(_program, name) {
      return name === "a_position" ? 0 : 1;
    },
    getProgramInfoLog() {
      return "";
    },
    getProgramParameter() {
      return true;
    },
    getShaderInfoLog() {
      return "driver rejected source";
    },
    getShaderParameter() {
      shaderChecks += 1;
      return shaderChecks !== options.failShaderAt;
    },
    getUniformLocation(_program, name) {
      return name === options.inactiveUniform ? null : {};
    },
    getError() {
      return options.uniformError ?? 0;
    },
    isTexture(value) {
      return value?.texture === true;
    },
    linkProgram() {},
    shaderSource() {},
    uniform2f() {},
    uniform1f(_location, value) {
      uniform1fValues.push(value);
    },
    uniform1i() {},
    uniform2fv() {},
    uniform3fv(_location, value) {
      uniform3fvValues.push([...value]);
    },
    uniform4fv() {},
    uniformMatrix2fv() {},
    uniformMatrix3fv() {},
    uniformMatrix4fv() {},
    useProgram() {},
    vertexAttribPointer() {},
    viewport() {},
  };
  return {
    context,
    draws,
    clears,
    uniform1fValues,
    uniform3fvValues,
    deletedShaders,
  };
}
