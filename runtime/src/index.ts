export { runtimeOps, runtimeSignatures } from "./generated/ops.js";
export { runtimeAbi, runtimeVersion } from "./generated/abi.js";
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
export {
  mapFromEntries,
  mapGet,
  mapSet,
  structFromEntries,
} from "./records.js";
export type { PolyglRecord } from "./records.js";
export { WebGL2BatchRenderer } from "./renderer.js";
export { WebGL2SceneRenderer } from "./scene.js";
export type {
  BasicMaterial,
  MaterialHandle,
  MeshHandle,
  NodeHandle,
  RuntimeImageLoader,
  SceneShaderValue,
  TextureHandle,
} from "./scene.js";
export { WebGL2ShaderRegistry } from "./shader.js";
export type {
  ShaderArtifact,
  ShaderAttribute,
  ShaderBundle,
  ShaderMaterial,
  NumericSequence,
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
  RuntimeResizeObserver,
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
import type { ShaderMaterial } from "./shader.js";
import type { NumericSequence } from "./shader.js";
import type {
  BasicMaterial,
  MaterialHandle,
  MeshHandle,
  NodeHandle,
  SceneShaderValue,
  TextureHandle,
} from "./scene.js";

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

export function stroke(r: number, g: number, b: number, a = 1): void {
  session().renderer.stroke(r, g, b, a);
}

export function noStroke(): void {
  session().renderer.noStroke();
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

export function line(
  x1: number,
  y1: number,
  x2: number,
  y2: number,
): void {
  session().renderer.line(x1, y1, x2, y2);
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

export function text(value: string, x: number, y: number): void {
  session().renderer.text(value, x, y);
}

export function pushMatrix(): void {
  session().renderer.pushMatrix();
}

export function popMatrix(): void {
  session().renderer.popMatrix();
}

export function translate(x: number, y: number): void {
  session().renderer.translate(x, y);
}

export function rotate(radians: number): void {
  session().renderer.rotate(radians);
}

export function scale(x: number, y: number): void {
  session().renderer.scale(x, y);
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

export function materialShader(shaderName: string): ShaderMaterial {
  return session().materialShader(shaderName);
}

export function meshBox(
  width: number,
  height: number,
  depth: number,
): MeshHandle {
  return session().meshBox(width, height, depth);
}

export function meshSphere(radius: number, segments: number): MeshHandle {
  return session().meshSphere(radius, segments);
}

export function meshPlane(
  width: number,
  depth: number,
  columns = 1,
  rows = 1,
): MeshHandle {
  return session().meshPlane(width, depth, columns, rows);
}

export function meshFrom(
  vertices: readonly number[],
  indices: readonly number[],
): MeshHandle {
  return session().meshFrom(vertices, indices);
}

export function materialBasic(color: NumericSequence): BasicMaterial {
  return session().materialBasic(color);
}

export function nodeAdd(
  mesh: MeshHandle,
  material: MaterialHandle,
): NodeHandle {
  return session().nodeAdd(mesh, material);
}

export function nodeRemove(node: NodeHandle): void {
  session().nodeRemove(node);
}

export function meshDispose(mesh: MeshHandle): void {
  session().meshDispose(mesh);
}

export function nodeSetPos(
  node: NodeHandle,
  x: number,
  y: number,
  z: number,
): void {
  session().nodeSetPosition(node, x, y, z);
}

export function nodeSetRot(
  node: NodeHandle,
  x: number,
  y: number,
  z: number,
): void {
  session().nodeSetRotation(node, x, y, z);
}

export function nodeSetScale(
  node: NodeHandle,
  x: number,
  y: number,
  z: number,
): void {
  session().nodeSetScale(node, x, y, z);
}

export function cameraPerspective(
  verticalFov: number,
  near: number,
  far: number,
): void {
  session().cameraPerspective(verticalFov, near, far);
}

export function cameraLookAt(
  eye: NumericSequence,
  target: NumericSequence,
  up: NumericSequence,
): void {
  session().cameraLookAt(eye, target, up);
}

export function lightDirectional(
  direction: NumericSequence,
  color: NumericSequence,
): void {
  session().lightDirectional(direction, color);
}

export function textureLoad(path: string): TextureHandle {
  return session().textureLoad(path);
}

export function textureDispose(texture: TextureHandle): void {
  session().textureDispose(texture);
}

export function shaderSet(
  node: NodeHandle,
  name: string,
  value: SceneShaderValue,
): void {
  session().shaderSet(node, name, value);
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
