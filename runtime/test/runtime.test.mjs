import assert from "node:assert/strict";
import test from "node:test";

import {
  SeededRandom,
  checkedIndex,
  circle,
  fill,
  formatRuntimeError,
  random,
  rect,
  roundToInt,
  runtimeOps,
  runtimeVersion,
  size,
  start,
  time,
  triangle,
  width,
} from "../dist/index.js";

test("exports generated runtime metadata", () => {
  assert.equal(runtimeOps.background, "background");
  assert.equal(runtimeOps.no_stroke, "noStroke");
  assert.equal(runtimeOps.time, "time");
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

function fakeWebGl2() {
  const draws = [];
  const clears = [];
  const context = {
    ARRAY_BUFFER: 0x8892,
    BLEND: 0x0be2,
    COLOR_BUFFER_BIT: 0x4000,
    COMPILE_STATUS: 0x8b81,
    DYNAMIC_DRAW: 0x88e8,
    FLOAT: 0x1406,
    FRAGMENT_SHADER: 0x8b30,
    LINK_STATUS: 0x8b82,
    ONE_MINUS_SRC_ALPHA: 0x0303,
    SRC_ALPHA: 0x0302,
    TRIANGLES: 0x0004,
    VERTEX_SHADER: 0x8b31,
    attachShader() {},
    bindBuffer() {},
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
    deleteShader() {},
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
      return "";
    },
    getShaderParameter() {
      return true;
    },
    getUniformLocation() {
      return {};
    },
    linkProgram() {},
    shaderSource() {},
    uniform2f() {},
    useProgram() {},
    vertexAttribPointer() {},
    viewport() {},
  };
  return { context, draws, clears };
}
