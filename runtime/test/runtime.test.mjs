import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const runtimeBundle = await readFile(
  new URL("../../crates/polygl-cli/assets/runtime.js", import.meta.url),
);
const {
  SeededRandom,
  background,
  checkedIndex,
  circle,
  fill,
  floorToInt,
  formatRuntimeError,
  keyDown,
  line,
  mapFromEntries,
  mapGet,
  mapSet,
  cameraLookAt,
  cameraPerspective,
  lightDirectional,
  materialBasic,
  materialShader,
  meshBox,
  meshDispose,
  meshFrom,
  meshPlane,
  meshSphere,
  mouseX,
  mouseY,
  noStroke,
  nodeAdd,
  nodeRemove,
  nodeSetPos,
  nodeSetRot,
  nodeSetScale,
  popMatrix,
  pushMatrix,
  random,
  rect,
  rotate,
  roundToInt,
  runtimeOps,
  runtimeAbi,
  shaderAbi,
  runtimeVersion,
  scale,
  setShaderUniform,
  shaderSet,
  size,
  start,
  stroke,
  structFromEntries,
  text,
  textureLoad,
  textureDispose,
  time,
  translate,
  triangle,
  truncToInt,
  width,
} = await import(`data:text/javascript;base64,${runtimeBundle.toString("base64")}`);

test("exports generated runtime metadata", () => {
  assert.equal(runtimeOps.background, "background");
  assert.equal(runtimeOps.no_stroke, "noStroke");
  assert.equal(runtimeOps.time, "time");
  assert.equal(runtimeOps.material_shader, "materialShader");
  assert.equal(runtimeVersion, "0.1.0");
  assert.equal(runtimeAbi, 2);
  assert.equal(shaderAbi, 1);
});

test("rejects generated programs with a missing or mismatched runtime ABI", async () => {
  const { context } = fakeWebGl2();
  const canvas = fakeCanvas();
  const options = { canvas, context, onError() {}, requireRuntimeAbi: true };

  await assert.rejects(start(async () => ({}), options), /ABI missing.*ABI 2/);
  await assert.rejects(
    start(async () => ({ __polyglRuntimeAbi: 1 }), options),
    /ABI 1.*ABI 2/,
  );
});

test("rejects a generated shader bundle with a missing or mismatched ABI", async () => {
  const program = { __polyglRuntimeAbi: runtimeAbi };
  for (const required of [undefined, shaderAbi + 1]) {
    const { context } = fakeWebGl2();
    await assert.rejects(
      start(program, {
        canvas: fakeCanvas(),
        context,
        onError() {},
        requireRuntimeAbi: true,
        shaderBundle: { debug: false, shaderAbi: required, shaders: [] },
      }),
      new RegExp(`shader ABI ${String(required ?? "missing")}.*shader ABI ${shaderAbi}`),
    );
  }
});

test("validates runtime options without invoking accessors", async () => {
  let getterCalls = 0;
  const accessorOptions = {};
  Object.defineProperty(accessorOptions, "canvas", {
    enumerable: true,
    get() {
      getterCalls += 1;
      return fakeCanvas();
    },
  });

  await assert.rejects(start({}, accessorOptions), /canvas.*data property/);
  assert.equal(getterCalls, 0);
  await assert.rejects(start({}, null), /runtime options must be a plain object/);
  await assert.rejects(
    start({}, Object.create({ canvas: fakeCanvas() })),
    /runtime options must not use a custom prototype/,
  );
  await assert.rejects(start({}, { seed: Number.NaN }), /seed.*finite number/);
  await assert.rejects(start({}, { autoResize: "yes" }), /autoResize.*boolean/);

  const { context } = fakeWebGl2();
  await assert.rejects(
    start({}, {
      canvas: fakeCanvas(),
      context,
      autoResize: true,
      createResizeObserver: () => ({}),
      onError() {},
    }),
    /resize observer\.observe.*function/,
  );
});

test("validates direct programs and program factory results", async () => {
  let getterCalls = 0;
  const accessorProgram = {};
  Object.defineProperty(accessorProgram, "setup", {
    enumerable: true,
    get() {
      getterCalls += 1;
      return () => {};
    },
  });
  await assert.rejects(start(accessorProgram), /program\.setup.*data property/);
  assert.equal(getterCalls, 0);
  await assert.rejects(start(null), /program must be a plain object/);
  await assert.rejects(
    start(Object.create({ setup() {} })),
    /program must not use a custom prototype/,
  );
  const forgedModule = {};
  Object.defineProperty(forgedModule, Symbol.toStringTag, {
    value: "Module",
    writable: false,
    enumerable: false,
    configurable: false,
  });
  await assert.rejects(
    start(forgedModule),
    /program must not contain symbol properties/,
  );
  const extensibleNullModule = Object.create(null);
  Object.defineProperty(extensibleNullModule, Symbol.toStringTag, {
    value: "Module",
    writable: false,
    enumerable: false,
    configurable: false,
  });
  await assert.rejects(
    start(extensibleNullModule),
    /program must not contain symbol properties/,
  );

  for (const result of [undefined, { frame: 1 }, { __polyglRuntimeAbi: 1.5 }]) {
    const { context } = fakeWebGl2();
    await assert.rejects(
      start(async () => result, {
        canvas: fakeCanvas(),
        context,
        onError() {},
      }),
      /program|runtime boundary\.frame/,
    );
  }
});

test("accepts a generated program loaded as an ES module namespace", async () => {
  const marker = "__polyglModuleNamespaceSetup";
  delete globalThis[marker];
  const moduleSource = [
    `export const __polyglRuntimeAbi = ${runtimeAbi};`,
    `export function setup() { globalThis.${marker} = true; }`,
    'export const unrelatedExport = "ignored";',
  ].join("\n");
  const moduleUrl = `data:text/javascript,${encodeURIComponent(moduleSource)}`;
  const namespace = await import(moduleUrl);
  assert.equal(namespace[Symbol.toStringTag], "Module");

  const { context } = fakeWebGl2();
  const handle = await start(() => import(moduleUrl), {
    canvas: fakeCanvas(),
    context,
    requireRuntimeAbi: true,
    onError() {},
  });
  assert.equal(globalThis[marker], true);
  handle.stop();
  delete globalThis[marker];
});

test("rejects malformed and ambiguous shader metadata before linking", async () => {
  let getterCalls = 0;
  const accessorBundle = { debug: false, shaders: [] };
  Object.defineProperty(accessorBundle, "shaderAbi", {
    get() {
      getterCalls += 1;
      return shaderAbi;
    },
  });
  await assert.rejects(
    start({}, { shaderBundle: accessorBundle }),
    /shader bundle\.shaderAbi.*data property/,
  );
  assert.equal(getterCalls, 0);

  const inheritedArtifact = shaderBundle([{ name: "inherited" }]);
  Object.setPrototypeOf(inheritedArtifact.shaders[0], { injected: true });
  await assert.rejects(
    start({}, { shaderBundle: inheritedArtifact }),
    /shaders\[0\].*custom prototype/,
  );

  const accessorArtifact = shaderBundle([{ name: "accessor" }]);
  Object.defineProperty(accessorArtifact.shaders[0], "uniforms", {
    enumerable: true,
    get() {
      getterCalls += 1;
      return [];
    },
  });
  await assert.rejects(
    start({}, { shaderBundle: accessorArtifact }),
    /shaders\[0\]\.uniforms.*data property/,
  );
  assert.equal(getterCalls, 0);

  const duplicateShaders = shaderBundle([
    { name: "repeated" },
    { name: "repeated" },
  ]);
  await assert.rejects(
    start({}, { shaderBundle: duplicateShaders }),
    /duplicate shader name `repeated`/,
  );

  const duplicateUniforms = shaderBundle([{
    name: "uniforms",
    uniforms: [
      { name: "tint", glslName: "pgl_u_tint", type: "vec3", source: "user" },
      { name: "tint", glslName: "pgl_u_other", type: "vec3", source: "user" },
    ],
  }]);
  await assert.rejects(
    start({}, { shaderBundle: duplicateUniforms }),
    /duplicate uniform name `tint`/,
  );

  const duplicateAttributes = shaderBundle([{
    name: "attributes",
    attributes: [
      { name: "position", glslName: "a_position", location: 0, type: "vec3" },
      { name: "position", glslName: "a_position", location: 0, type: "vec3" },
    ],
  }]);
  await assert.rejects(
    start({}, { shaderBundle: duplicateAttributes }),
    /duplicate attribute name `position`/,
  );

  for (const attribute of [
    { name: "position", glslName: "a_position", location: -1, type: "vec3" },
    { name: "position", glslName: "a_position", location: 0, type: "vec2" },
    { name: "unknown", glslName: "a_unknown", location: 0, type: "vec3" },
  ]) {
    await assert.rejects(
      start({}, {
        shaderBundle: shaderBundle([{
          name: "invalid_attribute",
          attributes: [attribute],
        }]),
      }),
      /location.*non-negative|invalid standard mesh attribute metadata/,
    );
  }

  await assert.rejects(
    start({}, {
      shaderBundle: shaderBundle([{
        name: "invalid_uniform",
        uniforms: [{
          name: "u_time",
          glslName: "u_time",
          type: "vec2",
          source: "automatic",
        }],
      }]),
    }),
    /invalid automatic uniform metadata/,
  );
});

test("checks shader metadata against linked program reflection", async () => {
  const position = {
    name: "position",
    glslName: "a_position",
    location: 0,
    type: "vec3",
  };
  const tint = {
    name: "tint",
    glslName: "pgl_u_tint",
    type: "vec3",
    source: "user",
  };

  const wrongLocation = fakeWebGl2({
    attributeLocations: { a_position: 2 },
  });
  await assert.rejects(
    start({}, {
      canvas: fakeCanvas(),
      context: wrongLocation.context,
      shaderBundle: shaderBundle([{
        name: "wrong_location",
        attributes: [position],
      }]),
      onError() {},
    }),
    /declares location 0.*reports 2/,
  );

  const optimizedAttribute = fakeWebGl2({
    attributeLocations: { a_position: [0, -1] },
  });
  const optimizedHandle = await start({}, {
    canvas: fakeCanvas(),
    context: optimizedAttribute.context,
    shaderBundle: shaderBundle([{
      name: "optimized_attribute",
      attributes: [position],
    }]),
    onError() {},
  });
  optimizedHandle.stop();

  const wrongAttributeType = fakeWebGl2({
    activeAttributes: [{ name: "a_position", size: 1, type: 0x8b50 }],
  });
  await assert.rejects(
    start({}, {
      canvas: fakeCanvas(),
      context: wrongAttributeType.context,
      shaderBundle: shaderBundle([{
        name: "wrong_attribute_type",
        attributes: [position],
      }]),
      onError() {},
    }),
    /attribute `position` type does not match/,
  );

  const wrongUniformType = fakeWebGl2({
    activeUniforms: [{ name: "pgl_u_tint", size: 1, type: 0x8b52 }],
  });
  await assert.rejects(
    start({}, {
      canvas: fakeCanvas(),
      context: wrongUniformType.context,
      shaderBundle: shaderBundle([{
        name: "wrong_uniform_type",
        uniforms: [tint],
      }]),
      onError() {},
    }),
    /uniform `tint` type does not match/,
  );

  const unrecordedUniform = fakeWebGl2({
    activeUniforms: [{ name: "driver_only", size: 1, type: 0x1406 }],
  });
  await assert.rejects(
    start({}, {
      canvas: fakeCanvas(),
      context: unrecordedUniform.context,
      shaderBundle: shaderBundle([{ name: "unrecorded" }]),
      onError() {},
    }),
    /unrecorded active uniform `driver_only`/,
  );

  const noTextureUnits = fakeWebGl2({ maxTextureUnits: 0 });
  await assert.rejects(
    start({}, {
      canvas: fakeCanvas(),
      context: noTextureUnits.context,
      shaderBundle: shaderBundle([{
        name: "too_many_samplers",
        uniforms: [{
          name: "albedo",
          glslName: "pgl_u_albedo",
          type: "texture",
          source: "user",
        }],
      }]),
      onError() {},
    }),
    /requires 1 texture units.*supports 0/,
  );
});

test("seeded random sequences are reproducible", () => {
  const first = new SeededRandom(42);
  const second = new SeededRandom(42);
  assert.deepEqual(
    [first.next(), first.next(), first.between(-2, 3)],
    [second.next(), second.next(), second.between(-2, 3)],
  );
});

test("float-to-int builtins define special values and saturate outside i32", () => {
  for (const convert of [floorToInt, roundToInt, truncToInt]) {
    assert.equal(convert(Number.NaN), 0);
    assert.equal(convert(Number.POSITIVE_INFINITY), 2_147_483_647);
    assert.equal(convert(Number.NEGATIVE_INFINITY), -2_147_483_648);
    assert.equal(convert(1e100), 2_147_483_647);
    assert.equal(convert(-1e100), -2_147_483_648);
    assert.equal(Object.is(convert(-0), -0), false);
  }
  assert.equal(floorToInt(-1.1), -2);
  assert.equal(roundToInt(-1.5), -2);
  assert.equal(truncToInt(-1.9), -1);
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

test("maps and structs isolate special keys from JavaScript prototypes", () => {
  const location = {
    source: "map.rb",
    line: 7,
    column: 3,
    start: 30,
    end: 41,
  };
  const map = mapFromEntries([
    ["__proto__", 1],
    ["constructor", 2],
    ["toString", 3],
    ["", 4],
    ["日本語", 5],
  ]);

  assert.equal(Object.getPrototypeOf(map), null);
  assert.equal(mapGet(map, "__proto__"), 1);
  assert.equal(mapGet(map, "constructor"), 2);
  assert.equal(mapGet(map, "toString"), 3);
  assert.equal(mapGet(map, ""), 4);
  assert.equal(mapGet(map, "日本語"), 5);
  const replacement = { polluted: true };
  assert.equal(mapSet(map, "__proto__", replacement), replacement);
  assert.equal(mapGet(map, "__proto__"), replacement);
  assert.equal({}.polluted, undefined);

  assert.throws(
    () => mapGet(map, "valueOf", location),
    (error) =>
      formatRuntimeError(error) ===
      'map.rb:7:3: map key "valueOf" is not present',
  );

  const struct = structFromEntries([
    ["__proto__", "field"],
    ["constructor", "also a field"],
  ]);
  assert.equal(Object.getPrototypeOf(struct), null);
  assert.equal(struct.__proto__, "field");
  assert.equal(struct.constructor, "also a field");
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

test("caps long frame gaps and coalesces event-driven renders", async () => {
  const animation = fakeWebGl2();
  const animationFrames = [];
  const deltas = [];
  const animationHandle = await start(
    { frame(dt) { deltas.push(dt); } },
    {
      canvas: fakeCanvas(),
      context: animation.context,
      maxDeltaSeconds: 0.05,
      requestAnimationFrame(callback) {
        animationFrames.push(callback);
        return animationFrames.length;
      },
      cancelAnimationFrame() {},
      onError() {},
    },
  );
  animationFrames[0](1_000);
  animationFrames[1](6_000);
  assert.deepEqual(deltas, [0, 0.05]);
  assert.equal(time(), 0.05);
  animationHandle.stop();

  const events = fakeWebGl2();
  const eventCanvas = fakeCanvas();
  const eventFrames = [];
  const eventHandle = await start(
    { on_event() {} },
    {
      canvas: eventCanvas,
      context: events.context,
      requestAnimationFrame(callback) {
        eventFrames.push(callback);
        return eventFrames.length;
      },
      cancelAnimationFrame() {},
      onError() {},
    },
  );
  for (let index = 0; index < 20; index += 1) {
    eventCanvas.listeners.get("pointermove")({ clientX: index, clientY: 1 });
  }
  assert.equal(eventFrames.length, 1);
  eventFrames[0](1_000);
  eventCanvas.listeners.get("pointermove")({ clientX: 20, clientY: 1 });
  assert.equal(eventFrames.length, 2);
  eventHandle.stop();
});

test("tracks display size and handles WebGL context loss deterministically", async () => {
  const resized = fakeWebGl2();
  const resizeCanvas = fakeCanvas();
  const resizeFrames = [];
  let resizeCallback;
  let observed;
  let disconnected = false;
  const resizeHandle = await start(
    {},
    {
      canvas: resizeCanvas,
      context: resized.context,
      autoResize: true,
      devicePixelRatio: 2,
      createResizeObserver(callback) {
        resizeCallback = callback;
        return {
          observe(target) { observed = target; },
          disconnect() { disconnected = true; },
        };
      },
      requestAnimationFrame(callback) {
        resizeFrames.push(callback);
        return resizeFrames.length;
      },
      cancelAnimationFrame() {},
      onError() {},
    },
  );
  assert.equal(observed, resizeCanvas);
  assert.equal(resizeCanvas.width, 128);
  assert.equal(resizeCanvas.height, 128);
  resizeCanvas.cssWidth = 100;
  resizeCanvas.cssHeight = 50;
  resizeCallback();
  assert.equal(resizeCanvas.width, 200);
  assert.equal(resizeCanvas.height, 100);
  assert.equal(resizeFrames.length, 1);
  resizeHandle.stop();
  assert.equal(disconnected, true);

  const lost = fakeWebGl2();
  const lostCanvas = fakeCanvas();
  const lostFrames = [];
  const cancelled = [];
  const failures = [];
  await start(
    { frame() {} },
    {
      canvas: lostCanvas,
      context: lost.context,
      requestAnimationFrame(callback) {
        lostFrames.push(callback);
        return lostFrames.length;
      },
      cancelAnimationFrame(handle) { cancelled.push(handle); },
      onError(reason) { failures.push(String(reason)); },
    },
  );
  let prevented = false;
  lostCanvas.listeners.get("webglcontextlost")({
    preventDefault() { prevented = true; },
  });
  assert.equal(prevented, true);
  assert.deepEqual(cancelled, [1]);
  assert.match(failures[0], /rendering is suspended/);
  lostCanvas.listeners.get("webglcontextrestored")({});
  assert.match(failures[1], /restart the runtime session/);
  assert.equal(lostCanvas.listeners.size, 0);
});

test("applies strokes and transforms while dispatching input events", async () => {
  const { context, uploads } = fakeWebGl2();
  const canvas = fakeCanvas();
  const documentObject = fakeDocument();
  const events = [];

  const handle = await start(
    {
      setup() {
        stroke(1, 0, 0);
        line(0, 0, 2, 0);
        noStroke();
        pushMatrix();
        translate(10, 20);
        rotate(0);
        scale(2, 3);
        stroke(0, 1, 0);
        line(0, 0, 1, 0);
        noStroke();
        triangle(0, 0, 1, 0, 0, 1);
        popMatrix();
      },
      on_event(event) {
        events.push({ ...event });
      },
    },
    {
      canvas,
      context,
      document: documentObject,
      onError() {},
    },
  );

  assert.equal(uploads.length, 1);
  const positions = [];
  for (let index = 0; index < uploads[0].length; index += 6) {
    positions.push(uploads[0].slice(index, index + 2));
  }
  assert.deepEqual(
    positions.slice(6, 12).map((position) => position[1]),
    [20.5, 20.5, 19.5, 20.5, 19.5, 19.5],
  );
  assert.deepEqual(positions.slice(-3), [[10, 20], [12, 20], [10, 23]]);
  assert.throws(
    () => popMatrix(),
    /pop_matrix called without a matching push_matrix/,
  );
  assert.throws(
    () => text("unavailable", 0, 0),
    /attached browser canvas with Canvas2D support/,
  );

  canvas.listeners.get("pointerdown")({
    clientX: 32,
    clientY: 16,
    pointerId: 7,
  });
  assert.equal(mouseX(), 32);
  assert.equal(mouseY(), 16);
  assert.equal(canvas.captured.has(7), true);
  canvas.listeners.get("pointerup")({
    clientX: 40,
    clientY: 24,
    pointerId: 7,
  });
  assert.equal(canvas.captured.has(7), false);
  documentObject.listeners.get("keydown")({ key: "ArrowLeft" });
  assert.equal(keyDown("ArrowLeft"), true);
  documentObject.defaultView.listeners.get("blur")();
  assert.equal(keyDown("ArrowLeft"), false);
  documentObject.listeners.get("keyup")({ key: "ArrowLeft" });
  assert.equal(keyDown("ArrowLeft"), false);
  assert.deepEqual(
    events.map((event) => event.kind),
    ["pointerdown", "pointerup", "keydown", "keyup"],
  );
  assert.deepEqual(events[0], {
    kind: "pointerdown",
    x: 32,
    y: 16,
    key: null,
  });
  handle.stop();
  assert.equal(canvas.listeners.size, 0);
  assert.equal(documentObject.listeners.size, 0);
});

test("draws text on a transformed Canvas2D overlay", async () => {
  const { context } = fakeWebGl2();
  const canvas = fakeCanvas();
  const overlay = fakeTextOverlay(canvas);
  const handle = await start(
    {
      setup() {
        fill(0.25, 0.5, 0.75, 0.8);
        pushMatrix();
        translate(3, 4);
        text("hello", 5, 6);
        popMatrix();
        background(0, 0, 0);
      },
    },
    {
      canvas,
      context,
      document: overlay.document,
      onError() {},
    },
  );

  assert.deepEqual(overlay.transforms, [[1, 0, 0, 1, 3, 4]]);
  assert.deepEqual(overlay.texts, [["hello", 5, 6]]);
  assert.equal(overlay.fillStyles[0], "rgba(63.75, 127.5, 191.25, 0.8)");
  assert.deepEqual(overlay.clears, [[0, 0, 64, 64]]);
  handle.stop();
  assert.equal(overlay.removed, true);
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
        const tint = new Float32Array([0.2, 0.4, 0.6]);
        setShaderUniform("plasma", "tint", tint);
        tint[0] = Number.NaN;
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
  assert.deepEqual(uniform3fvValues, [
    Array.from(new Float32Array([0.2, 0.4, 0.6])),
  ]);
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

test("renders retained 3D primitives and rejects handles from old sessions", async () => {
  const {
    context,
    elementDraws,
    uniformMatrix4Values,
    uploads,
  } = fakeWebGl2();
  let oldNode;
  const handle = await start(
    {
      setup() {
        size(400, 240);
        cameraPerspective(Math.PI / 3, 0.1, 50);
        cameraLookAt(
          new Float32Array([4, 3, 6]),
          new Float32Array([0, 0, 0]),
          new Float32Array([0, 1, 0]),
        );
        lightDirectional(
          new Float32Array([-1, -2, -1]),
          new Float32Array([1, 0.9, 0.8]),
        );
        const material = materialBasic(
          new Float32Array([0.2, 0.6, 0.9, 1]),
        );
        oldNode = nodeAdd(meshBox(1, 2, 3), material);
        nodeSetPos(oldNode, -2, 0, 0);
        nodeSetRot(oldNode, 0.1, 0.2, 0.3);
        nodeSetScale(oldNode, 1, 1.5, 0.75);
        nodeAdd(meshSphere(1, 8), material);
        nodeAdd(meshPlane(4, 3, 2, 3), material);
        nodeAdd(
          meshFrom(
            [
              0, 0, 0, 0, 0, 1, 0, 0, 1, 1, 1, 1,
              1, 0, 0, 0, 0, 1, 1, 0, 1, 1, 1, 1,
              0, 1, 0, 0, 0, 1, 0, 1, 1, 1, 1, 1,
            ],
            [0, 1, 2],
          ),
          material,
        );
      },
    },
    { canvas: fakeCanvas(), context, onError() {} },
  );

  assert.deepEqual(elementDraws, [36, 192, 36, 3]);
  assert.equal(uploads.some((upload) => upload.length === 24 * 12), true);
  const modelUploads = uniformMatrix4Values.filter(
    (upload) => upload.name === "u_model",
  );
  assert.equal(modelUploads.length, 4);
  assert.equal(modelUploads[0].value[12], -2);
  assert.throws(
    () => meshFrom([0, 1, 2], [0, 1, 2]),
    /12 finite values per vertex/,
  );
  handle.stop();

  const next = fakeWebGl2();
  await assert.rejects(
    start(
      {
        setup() {
          nodeSetRot(oldNode, 0, 0, 0);
        },
      },
      { canvas: fakeCanvas(), context: next.context, onError() {} },
    ),
    /another runtime session/,
  );
});

test("uploads custom shader uniforms independently for each node", async () => {
  const {
    context,
    elementDraws,
    uniform3fvValues,
    uniformMatrix4Values,
  } = fakeWebGl2();
  const bundle = shaderBundle([
    {
      name: "lit",
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
          name: "u_model",
          glslName: "u_model",
          type: "mat4",
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
      setup() {
        const mesh = meshBox(1, 1, 1);
        const material = materialShader("lit");
        const left = nodeAdd(mesh, material);
        const right = nodeAdd(mesh, material);
        nodeSetPos(left, -1.5, 0, 0);
        nodeSetPos(right, 1.5, 0, 0);
        shaderSet(left, "tint", new Float32Array([1, 0, 0]));
        shaderSet(right, "tint", new Float32Array([0, 0, 1]));
      },
    },
    {
      canvas: fakeCanvas(),
      context,
      shaderBundle: bundle,
      onError() {},
    },
  );

  assert.deepEqual(elementDraws, [36, 36]);
  assert.deepEqual(uniform3fvValues.slice(-2), [[1, 0, 0], [0, 0, 1]]);
  const models = uniformMatrix4Values.filter(
    (upload) => upload.name === "u_model",
  );
  assert.equal(models.at(-2).value[12], -1.5);
  assert.equal(models.at(-1).value[12], 1.5);
  handle.stop();
});

test("keeps GPU resources alive while referenced and invalidates disposed handles", async () => {
  const gl = fakeWebGl2();
  let mesh;
  let node;
  let texture;
  const handle = await start(
    {
      setup() {
        mesh = meshBox(1, 1, 1);
        texture = textureLoad("assets/albedo.png");
        node = nodeAdd(mesh, materialShader("textured"));
        shaderSet(node, "albedo", texture);
      },
    },
    {
      canvas: fakeCanvas(),
      context: gl.context,
      shaderBundle: shaderBundle([{
        name: "textured",
        uniforms: [{
          name: "albedo",
          glslName: "pgl_u_albedo",
          type: "texture",
          source: "user",
        }],
      }]),
      imageLoader: async () => ({ width: 2, height: 2 }),
      onError() {},
    },
  );

  assert.equal(Object.isFrozen(mesh), true);
  assert.equal(Object.isFrozen(node), true);
  assert.equal(Object.isFrozen(texture), true);
  assert.throws(() => { mesh.kind = "forged"; }, TypeError);
  assert.throws(() => meshDispose(mesh), /referenced by 1 scene node/);
  assert.throws(() => textureDispose(texture), /referenced by 1 scene node uniform/);
  nodeRemove(node);
  assert.throws(() => nodeSetPos(node, 0, 0, 0), /no longer valid/);
  meshDispose(mesh);
  textureDispose(texture);
  assert.throws(() => meshDispose(mesh), /no longer valid/);
  assert.throws(() => textureDispose(texture), /no longer valid/);
  assert.equal(gl.deletedBuffers.length, 2);
  assert.equal(gl.deletedTextures.length, 1);
  handle.stop();
  assert.equal(gl.deletedBuffers.length, 3);
  assert.equal(gl.deletedTextures.length, 1);
});

test("cancels a pending texture upload when its handle is disposed", async () => {
  const gl = fakeWebGl2();
  let rejectImage;
  let texture;
  const started = start(
    {
      setup() {
        texture = textureLoad("assets/transient.png");
        textureDispose(texture);
      },
    },
    {
      canvas: fakeCanvas(),
      context: gl.context,
      imageLoader() {
        return new Promise((_resolve, reject) => { rejectImage = reject; });
      },
      onError() {},
    },
  );
  await Promise.resolve();
  rejectImage(new Error("late network failure"));
  const handle = await started;
  assert.equal(texture.loaded, false);
  assert.equal(gl.textureUploads.length, 1);
  assert.equal(gl.deletedTextures.length, 1);
  handle.stop();
});

test("waits for setup textures and uses a white placeholder during frames", async () => {
  const setupGl = fakeWebGl2();
  const setupDocument = fakeDocument();
  setupDocument.baseURI = "https://example.test/demo/";
  let finishSetupImage;
  let setupTexture;
  const setupUrls = [];
  const setupStarted = start(
    {
      setup() {
        setupTexture = textureLoad("assets/grid.png");
      },
    },
    {
      canvas: fakeCanvas(),
      context: setupGl.context,
      document: setupDocument,
      imageLoader(url) {
        setupUrls.push(url);
        return new Promise((resolve) => {
          finishSetupImage = resolve;
        });
      },
      onError() {},
    },
  );
  let setupSettled = false;
  void setupStarted.then(() => {
    setupSettled = true;
  });
  await Promise.resolve();
  await Promise.resolve();
  assert.equal(setupSettled, false);
  assert.equal(setupTexture.loaded, false);
  assert.deepEqual(setupUrls, ["https://example.test/demo/assets/grid.png"]);
  assert.deepEqual(
    [...setupGl.textureUploads[0].at(-1)],
    [255, 255, 255, 255],
  );
  finishSetupImage({ width: 2, height: 2 });
  const setupHandle = await setupStarted;
  assert.equal(setupTexture.loaded, true);
  assert.equal(setupGl.textureUploads.length, 2);
  setupHandle.stop();

  const frameGl = fakeWebGl2();
  const frames = [];
  let finishFrameImage;
  let frameTexture;
  const frameHandle = await start(
    {
      frame() {
        frameTexture ??= textureLoad("assets/dynamic.png");
      },
    },
    {
      canvas: fakeCanvas(),
      context: frameGl.context,
      requestAnimationFrame(callback) {
        frames.push(callback);
        return frames.length;
      },
      cancelAnimationFrame() {},
      imageLoader() {
        return new Promise((resolve) => {
          finishFrameImage = resolve;
        });
      },
      onError() {},
    },
  );
  frames[0](1000);
  assert.equal(frameTexture.loaded, false);
  assert.deepEqual(
    [...frameGl.textureUploads[0].at(-1)],
    [255, 255, 255, 255],
  );
  finishFrameImage({ width: 4, height: 4 });
  await Promise.resolve();
  await Promise.resolve();
  assert.equal(frameTexture.loaded, true);
  assert.equal(frameGl.textureUploads.length, 2);
  frameHandle.stop();
});

test("turns setup texture failures into startup failures", async () => {
  const { context } = fakeWebGl2();
  const canvas = fakeCanvas();
  const failures = [];
  await assert.rejects(
    start(
      {
        setup() {
          textureLoad("assets/missing.png");
        },
      },
      {
        canvas,
        context,
        imageLoader: async () => {
          throw new Error("not found");
        },
        onError(reason) {
          failures.push(reason);
        },
      },
    ),
    /failed to load texture `assets\/missing.png`: not found/,
  );
  assert.equal(failures.length, 1);
  assert.equal(canvas.listeners.size, 0);
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
        setup() {
          const node = nodeAdd(
            meshBox(1, 1, 1),
            materialShader("needs_color"),
          );
          nodeSetRot(node, 0, 0.25, 0);
        },
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
  const captured = new Set();
  return {
    width: 64,
    height: 64,
    cssWidth: 64,
    cssHeight: 64,
    captured,
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
      return {
        left: 0,
        top: 0,
        width: this.cssWidth,
        height: this.cssHeight,
      };
    },
    hasPointerCapture(pointerId) {
      return captured.has(pointerId);
    },
    releasePointerCapture(pointerId) {
      captured.delete(pointerId);
    },
    setPointerCapture(pointerId) {
      captured.add(pointerId);
    },
  };
}

function fakeDocument() {
  const listeners = new Map();
  const windowListeners = new Map();
  return {
    defaultView: {
      listeners: windowListeners,
      addEventListener(name, listener) {
        windowListeners.set(name, listener);
      },
      removeEventListener(name, listener) {
        if (windowListeners.get(name) === listener) {
          windowListeners.delete(name);
        }
      },
    },
    listeners,
    addEventListener(name, listener) {
      listeners.set(name, listener);
    },
    removeEventListener(name, listener) {
      if (listeners.get(name) === listener) {
        listeners.delete(name);
      }
    },
  };
}

function fakeTextOverlay(canvas) {
  const transforms = [];
  const texts = [];
  const fillStyles = [];
  const clears = [];
  const documentObject = fakeDocument();
  let removed = false;
  const context = {
    clearRect(...args) {
      clears.push(args);
    },
    fillText(...args) {
      texts.push(args);
    },
    restore() {},
    save() {},
    set fillStyle(value) {
      fillStyles.push(value);
    },
    set textBaseline(_value) {},
    setTransform(...args) {
      transforms.push(args);
    },
  };
  const overlayCanvas = {
    width: 0,
    height: 0,
    id: "",
    style: {},
    getContext(name) {
      return name === "2d" ? context : null;
    },
    remove() {
      removed = true;
    },
    setAttribute() {},
  };
  const wrapper = {
    style: {},
    append(target, overlay) {
      assert.equal(target, canvas);
      assert.equal(overlay, overlayCanvas);
    },
    replaceWith(target) {
      assert.equal(target, canvas);
      removed = true;
    },
  };
  canvas.id = "test-canvas";
  canvas.style = {};
  canvas.parentElement = {
    insertBefore(inserted, target) {
      assert.equal(inserted, wrapper);
      assert.equal(target, canvas);
    },
  };
  documentObject.createElement = (name) => {
    if (name === "canvas") {
      return overlayCanvas;
    }
    assert.equal(name, "div");
    return wrapper;
  };
  return {
    clears,
    document: documentObject,
    fillStyles,
    get removed() {
      return removed;
    },
    texts,
    transforms,
  };
}

function fakeWebGl2(options = {}) {
  const draws = [];
  const elementDraws = [];
  const clears = [];
  const uploads = [];
  const textureUploads = [];
  const uniform1fValues = [];
  const uniform3fvValues = [];
  const uniformMatrix4Values = [];
  const deletedShaders = [];
  const deletedBuffers = [];
  const deletedTextures = [];
  let shaderChecks = 0;
  const attributeLocationCalls = new Map();
  const activeAttributes = options.activeAttributes ?? [];
  const activeUniforms = options.activeUniforms ?? [];
  const context = {
    ACTIVE_ATTRIBUTES: 0x8b89,
    ACTIVE_UNIFORMS: 0x8b86,
    ARRAY_BUFFER: 0x8892,
    BLEND: 0x0be2,
    BOOL: 0x8b56,
    CLAMP_TO_EDGE: 0x812f,
    COLOR_BUFFER_BIT: 0x4000,
    COMPILE_STATUS: 0x8b81,
    DEPTH_BUFFER_BIT: 0x0100,
    DEPTH_TEST: 0x0b71,
    DYNAMIC_DRAW: 0x88e8,
    ELEMENT_ARRAY_BUFFER: 0x8893,
    FLOAT: 0x1406,
    FLOAT_MAT2: 0x8b5a,
    FLOAT_MAT3: 0x8b5b,
    FLOAT_MAT4: 0x8b5c,
    FLOAT_VEC2: 0x8b50,
    FLOAT_VEC3: 0x8b51,
    FLOAT_VEC4: 0x8b52,
    FRAGMENT_SHADER: 0x8b30,
    LEQUAL: 0x0203,
    LINEAR: 0x2601,
    LINK_STATUS: 0x8b82,
    INT: 0x1404,
    MAX_TEXTURE_IMAGE_UNITS: 0x8872,
    NO_ERROR: 0,
    ONE_MINUS_SRC_ALPHA: 0x0303,
    RGBA: 0x1908,
    SAMPLER_2D: 0x8b5e,
    SRC_ALPHA: 0x0302,
    STATIC_DRAW: 0x88e4,
    TEXTURE_MAG_FILTER: 0x2800,
    TEXTURE_MIN_FILTER: 0x2801,
    TEXTURE_WRAP_S: 0x2802,
    TEXTURE_WRAP_T: 0x2803,
    TRIANGLES: 0x0004,
    TEXTURE0: 0x84c0,
    TEXTURE_2D: 0x0de1,
    UNSIGNED_BYTE: 0x1401,
    UNSIGNED_INT: 0x1405,
    VERTEX_SHADER: 0x8b31,
    activeTexture() {},
    attachShader() {},
    bindBuffer() {},
    bindTexture() {},
    blendFunc() {},
    bufferData(_target, data) {
      uploads.push([...data]);
    },
    clear(mask) {
      clears.push(mask);
    },
    clearColor() {},
    compileShader() {},
    createTexture() {
      return { texture: true };
    },
    createBuffer() {
      return {};
    },
    createProgram() {
      return {};
    },
    createShader() {
      return {};
    },
    deleteBuffer(buffer) { deletedBuffers.push(buffer); },
    deleteProgram() {},
    deleteShader(shader) {
      deletedShaders.push(shader);
    },
    deleteTexture(texture) { deletedTextures.push(texture); },
    depthFunc() {},
    disable() {},
    drawArrays(_mode, _first, count) {
      draws.push(count);
    },
    drawElements(_mode, count) {
      elementDraws.push(count);
    },
    enable() {},
    enableVertexAttribArray() {},
    getActiveAttrib(_program, index) {
      return activeAttributes[index] ?? null;
    },
    getActiveUniform(_program, index) {
      return activeUniforms[index] ?? null;
    },
    getAttribLocation(_program, name) {
      const configured = options.attributeLocations?.[name];
      if (Array.isArray(configured)) {
        const call = attributeLocationCalls.get(name) ?? 0;
        attributeLocationCalls.set(name, call + 1);
        return configured[Math.min(call, configured.length - 1)];
      }
      if (configured !== undefined) {
        return configured;
      }
      return name === "a_position" ? 0 : 1;
    },
    getParameter(parameter) {
      if (parameter === 0x8872) return options.maxTextureUnits ?? 16;
      return null;
    },
    getProgramInfoLog() {
      return "";
    },
    getProgramParameter(_program, parameter) {
      if (parameter === 0x8b89) return activeAttributes.length;
      if (parameter === 0x8b86) return activeUniforms.length;
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
      return name === options.inactiveUniform ? null : { name };
    },
    getError() {
      return options.uniformError ?? 0;
    },
    isTexture(value) {
      return value?.texture === true;
    },
    linkProgram() {},
    shaderSource() {},
    texImage2D(...args) {
      textureUploads.push(args);
    },
    texParameteri() {},
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
    uniformMatrix4fv(location, _transpose, value) {
      uniformMatrix4Values.push({
        name: location.name,
        value: [...value],
      });
    },
    useProgram() {},
    vertexAttribPointer() {},
    viewport() {},
  };
  return {
    context,
    draws,
    elementDraws,
    clears,
    uploads,
    textureUploads,
    uniform1fValues,
    uniform3fvValues,
    uniformMatrix4Values,
    deletedShaders,
    deletedBuffers,
    deletedTextures,
  };
}
