export { runtimeOps, runtimeSignatures } from "./generated/ops.js";
export type {
  BuiltinName,
  RuntimeDomain,
  RuntimeOp,
  RuntimeParameterSpec,
  RuntimeValueType,
} from "./generated/ops.js";
export {
  formatRuntimeError,
  runtimeError,
  showRuntimeError,
} from "./errors.js";
export type { LocatedError, SourceLocation } from "./errors.js";
export { SeededRandom } from "./random.js";
export { WebGL2BatchRenderer } from "./renderer.js";
export { WebGL2ShaderRegistry } from "./shader.js";
export type {
  ShaderArtifact,
  ShaderAttribute,
  ShaderBundle,
  ShaderUniform,
  ShaderUniformValue,
  ShaderValueType,
} from "./shader.js";
export { RuntimeSession } from "./session.js";
export type {
  PolyglProgram,
  PolyglProgramLoader,
  PolyglProgramSource,
  RuntimeEvent,
  RuntimeHandle,
  RuntimeOptions,
} from "./session.js";

import { runtimeError } from "./errors.js";
import type { SourceLocation } from "./errors.js";
import { RuntimeSession } from "./session.js";
import type {
  RuntimeHandle,
  RuntimeOptions,
  PolyglProgramSource,
} from "./session.js";
import type { ShaderUniformValue } from "./shader.js";

export const runtimeVersion = "0.0.0" as const;

let activeSession: RuntimeSession | undefined;
let startInProgress = false;

export async function start(
  source: PolyglProgramSource,
  options: RuntimeOptions = {},
): Promise<RuntimeHandle> {
  if (startInProgress) {
    throw new Error("a PolyGL runtime is already starting");
  }
  startInProgress = true;
  try {
    activeSession?.stop();
    const canvas =
      options.canvas ?? createCanvas(options.document ?? globalThis.document);
    const newSession = new RuntimeSession(canvas, options);
    activeSession = newSession;
    newSession.setStopHandler(() => {
      if (activeSession === newSession) {
        activeSession = undefined;
      }
    });
    await newSession.run(source);
    return newSession;
  } finally {
    startInProgress = false;
  }
}

export function size(width: number, height: number): void {
  session().renderer.resize(width, height);
}

export function background(r: number, g: number, b: number): void {
  session().renderer.background(r, g, b);
}

export function fill(r: number, g: number, b: number, a = 1): void {
  session().renderer.fill(r, g, b, a);
}

export function rect(
  x: number,
  y: number,
  width: number,
  height: number,
): void {
  session().renderer.rect(x, y, width, height);
}

export function circle(x: number, y: number, radius: number): void {
  session().renderer.circle(x, y, radius);
}

export function triangle(
  x1: number,
  y1: number,
  x2: number,
  y2: number,
  x3: number,
  y3: number,
): void {
  session().renderer.triangle(x1, y1, x2, y2, x3, y3);
}

export function width(): number {
  return session().canvas.width | 0;
}

export function height(): number {
  return session().canvas.height | 0;
}

export function time(): number {
  return session().elapsedSeconds;
}

export function mouseX(): number {
  return session().mouseX;
}

export function mouseY(): number {
  return session().mouseY;
}

export function keyDown(key: string): boolean {
  return session().keyDown(key);
}

export function random(a: number, b: number): number {
  return session().randomSource.between(a, b);
}

export function setShaderUniform(
  shaderName: string,
  uniformName: string,
  value: ShaderUniformValue,
): void {
  session().setShaderUniform(shaderName, uniformName, value);
}

export function floorToInt(value: number): number {
  return Math.floor(value) | 0;
}

export function roundToInt(value: number): number {
  return (value < 0 ? Math.ceil(value - 0.5) : Math.floor(value + 0.5)) | 0;
}

export function truncToInt(value: number): number {
  return Math.trunc(value) | 0;
}

export function checkedIndex<T>(
  collection: ArrayLike<T> | null | undefined,
  index: number,
  location?: SourceLocation,
): T {
  checkIndex(collection, index, location);
  return collection[index] as T;
}

export function checkIndex<T>(
  collection: ArrayLike<T> | null | undefined,
  index: number,
  location?: SourceLocation,
): asserts collection is ArrayLike<T> {
  if (collection === null || collection === undefined) {
    throw runtimeError("cannot index nil", location);
  }
  if (!Number.isInteger(index) || index < 0 || index >= collection.length) {
    throw runtimeError(
      `index ${index} is outside 0..${Math.max(0, collection.length - 1)}`,
      location,
    );
  }
}

export function requireNonNil<T>(
  value: T | null | undefined,
  location?: SourceLocation,
): T {
  if (value === null || value === undefined) {
    throw runtimeError("cannot access a field on nil", location);
  }
  return value;
}

function session(): RuntimeSession {
  if (activeSession === undefined) {
    throw new Error("the PolyGL runtime has not been started");
  }
  return activeSession;
}

function createCanvas(documentObject: Document | undefined): HTMLCanvasElement {
  if (documentObject === undefined) {
    throw new Error("start() requires a canvas outside a browser document");
  }
  const existing = documentObject.getElementById("polygl-canvas");
  if (existing !== null && existing.tagName.toLowerCase() === "canvas") {
    return existing as HTMLCanvasElement;
  }
  const canvas = documentObject.createElement("canvas");
  canvas.id = "polygl-canvas";
  canvas.width = 640;
  canvas.height = 480;
  documentObject.body.append(canvas);
  return canvas;
}
