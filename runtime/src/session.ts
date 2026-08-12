import { showRuntimeError } from "./errors.js";
import { runtimeAbi } from "./generated/abi.js";
import { SeededRandom } from "./random.js";
import { WebGL2BatchRenderer } from "./renderer.js";
import { WebGL2SceneRenderer } from "./scene.js";
import type {
  BasicMaterial,
  MaterialHandle,
  MeshHandle,
  NodeHandle,
  RuntimeImageLoader,
  SceneShaderValue,
  TextureHandle,
} from "./scene.js";
import { WebGL2ShaderRegistry } from "./shader.js";
import type {
  NumericSequence,
  ShaderBundle,
  ShaderMaterial,
  ShaderUniformValue,
} from "./shader.js";
import {
  validateProgram,
  validateResizeObserver,
  validateRuntimeOptions,
} from "./validation.js";

export interface RuntimeEvent {
  readonly kind: string;
  readonly x: number;
  readonly y: number;
  readonly key: string | null;
  readonly code: string | null;
  readonly pointerId: number | null;
  readonly buttons: number;
  readonly button: number;
  readonly pressure: number;
  readonly wheelX: number;
  readonly wheelY: number;
  readonly altKey: boolean;
  readonly ctrlKey: boolean;
  readonly metaKey: boolean;
  readonly shiftKey: boolean;
  readonly inside: boolean;
}

export type RuntimeSessionState =
  | "initializing"
  | "running"
  | "paused"
  | "context-lost"
  | "stopped";

export interface PolyglProgram {
  readonly setup?: () => void | Promise<void>;
  readonly frame?: (dt: number) => void;
  readonly on_event?: (event: RuntimeEvent) => void;
  readonly __polyglShaderBundle?: ShaderBundle;
  readonly __polyglRuntimeAbi?: number;
}

export type PolyglProgramLoader = () => Promise<PolyglProgram>;
export type PolyglProgramSource = PolyglProgram | PolyglProgramLoader;

export interface RuntimeResizeObserver {
  observe(target: Element): void;
  disconnect(): void;
}

export interface RuntimeOptions {
  readonly canvas?: HTMLCanvasElement;
  readonly context?: WebGL2RenderingContext;
  readonly document?: Document;
  readonly requestAnimationFrame?: (callback: FrameRequestCallback) => number;
  readonly cancelAnimationFrame?: (handle: number) => void;
  readonly seed?: number;
  readonly shaderBundle?: ShaderBundle;
  readonly imageLoader?: RuntimeImageLoader;
  readonly onError?: (reason: unknown) => void;
  readonly requireRuntimeAbi?: boolean;
  readonly maxDeltaSeconds?: number;
  readonly fixedDeltaSeconds?: number;
  readonly maxCatchUpSteps?: number;
  readonly visibilityPolicy?: "continue" | "pause";
  readonly autoResize?: boolean;
  readonly devicePixelRatio?: number;
  readonly createResizeObserver?: (
    callback: () => void,
  ) => RuntimeResizeObserver;
  readonly focusOnPointerDown?: boolean;
  readonly preventDefaultInput?: boolean;
}

export interface RuntimeHandle {
  readonly canvas: HTMLCanvasElement;
  stop(): void;
}

export class RuntimeSession implements RuntimeHandle {
  public readonly renderer: WebGL2BatchRenderer;
  public readonly scene: WebGL2SceneRenderer;
  public readonly randomSource: SeededRandom;
  public mouseX = 0;
  public mouseY = 0;
  public elapsedSeconds = 0;
  private sessionState: RuntimeSessionState = "initializing";
  private readonly pressedKeys = new Set<string>();
  private readonly documentObject: Document | undefined;
  private readonly windowObject: Window | undefined;
  private readonly requestFrame: (callback: FrameRequestCallback) => number;
  private readonly cancelFrame: (handle: number) => void;
  private readonly onError: (reason: unknown) => void;
  private readonly maxDeltaSeconds: number;
  private readonly fixedDeltaSeconds: number | undefined;
  private readonly maxCatchUpSteps: number;
  private readonly visibilityPolicy: "continue" | "pause";
  private readonly focusOnPointerDown: boolean;
  private readonly preventDefaultInput: boolean;
  private readonly autoResize: boolean;
  private readonly configuredDevicePixelRatio: number | undefined;
  private animationHandle: number | undefined;
  private renderHandle: number | undefined;
  private previousTimestamp: number | undefined;
  private fixedAccumulator = 0;
  private resizeObserver: RuntimeResizeObserver | undefined;
  private contextLost = false;
  private stopped = false;
  private onStop: () => void = () => {};
  private shaderRegistry: WebGL2ShaderRegistry;
  private readonly initialShaderBundle: ShaderBundle | undefined;
  private readonly requireRuntimeAbi: boolean;

  public get state(): RuntimeSessionState {
    return this.sessionState;
  }

  public constructor(
    public readonly canvas: HTMLCanvasElement,
    options: RuntimeOptions,
  ) {
    options = validateRuntimeOptions(options);
    this.maxDeltaSeconds = positiveFinite(
      options.maxDeltaSeconds ?? 0.1,
      "maxDeltaSeconds",
    );
    this.fixedDeltaSeconds = options.fixedDeltaSeconds;
    this.maxCatchUpSteps = options.maxCatchUpSteps ?? 8;
    this.visibilityPolicy = options.visibilityPolicy ?? "continue";
    this.focusOnPointerDown = options.focusOnPointerDown ?? false;
    this.preventDefaultInput = options.preventDefaultInput ?? false;
    this.configuredDevicePixelRatio = options.devicePixelRatio === undefined
      ? undefined
      : positiveFinite(options.devicePixelRatio, "devicePixelRatio");
    this.autoResize = options.autoResize ?? false;
    this.documentObject = options.document ?? globalThis.document;
    this.windowObject = this.documentObject?.defaultView ?? undefined;
    this.renderer = new WebGL2BatchRenderer(
      canvas,
      options.context,
      this.documentObject,
    );
    this.shaderRegistry = WebGL2ShaderRegistry.fromBundle(
      this.renderer.context,
    );
    this.scene = new WebGL2SceneRenderer(
      this.renderer.context,
      this.shaderRegistry,
      this.documentObject,
      options.imageLoader,
      (reason) => this.fail(reason),
    );
    this.initialShaderBundle = options.shaderBundle;
    this.requireRuntimeAbi = options.requireRuntimeAbi ?? false;
    this.randomSource = new SeededRandom(options.seed);
    const frameScheduler = resolveFrameScheduler(options);
    this.requestFrame = frameScheduler.request;
    this.cancelFrame = frameScheduler.cancel;
    this.onError =
      options.onError ??
      ((reason) => showRuntimeError(reason, this.documentObject));
    this.installInputListeners();
    this.installContextListeners();
    if (this.autoResize) {
      try {
        this.installResizeObserver(options.createResizeObserver);
        this.syncDisplaySize();
      } catch (error) {
        this.stop();
        throw error;
      }
    }
  }

  public async run(source: PolyglProgramSource): Promise<void> {
    try {
      this.replaceShaderBundle(this.initialShaderBundle, this.requireRuntimeAbi);
      const program = validateProgram(
        typeof source === "function" ? await source() : source,
      );
      if (this.stopped) {
        return;
      }
      if (
        (this.requireRuntimeAbi || program.__polyglRuntimeAbi !== undefined) &&
        program.__polyglRuntimeAbi !== runtimeAbi
      ) {
        throw new Error(
          `generated program requires runtime ABI ${String(program.__polyglRuntimeAbi ?? "missing")}; this runtime provides ABI ${runtimeAbi}`,
        );
      }
      this.program = program;
      if (program.__polyglShaderBundle !== undefined) {
        this.replaceShaderBundle(
          program.__polyglShaderBundle,
          this.requireRuntimeAbi || program.__polyglRuntimeAbi !== undefined,
        );
      }
      await program.setup?.();
      await this.scene.awaitSetupAssets();
      if (this.stopped) {
        return;
      }
      if (!this.contextLost) {
        this.render();
      }
      this.sessionState = this.shouldPauseForVisibility() ? "paused" : "running";
      if (
        program.frame !== undefined &&
        !this.contextLost &&
        this.sessionState === "running"
      ) {
        this.animationHandle = this.requestFrame(this.tick);
      }
    } catch (error) {
      this.fail(error);
      throw error;
    }
  }

  public stop(): void {
    if (this.stopped) {
      return;
    }
    this.stopped = true;
    this.sessionState = "stopped";
    if (this.animationHandle !== undefined) {
      this.cancelFrame(this.animationHandle);
      this.animationHandle = undefined;
    }
    if (this.renderHandle !== undefined) {
      this.cancelFrame(this.renderHandle);
      this.renderHandle = undefined;
    }
    this.resizeObserver?.disconnect();
    this.resizeObserver = undefined;
    this.canvas.removeEventListener("pointermove", this.handlePointerMove);
    this.canvas.removeEventListener("pointerdown", this.handlePointerDown);
    this.canvas.removeEventListener("pointerup", this.handlePointerUp);
    this.canvas.removeEventListener("pointercancel", this.handlePointerCancel);
    this.canvas.removeEventListener("wheel", this.handleWheel);
    this.canvas.removeEventListener("webglcontextlost", this.handleContextLost);
    this.canvas.removeEventListener(
      "webglcontextrestored",
      this.handleContextRestored,
    );
    this.documentObject?.removeEventListener("keydown", this.handleKeyDown);
    this.documentObject?.removeEventListener("keyup", this.handleKeyUp);
    this.documentObject?.removeEventListener(
      "visibilitychange",
      this.handleVisibilityChange,
    );
    this.windowObject?.removeEventListener("blur", this.handleBlur);
    this.renderer.dispose();
    this.scene.dispose();
    this.shaderRegistry.dispose();
    this.onStop();
  }

  public keyDown(key: string): boolean {
    return this.pressedKeys.has(key);
  }

  public setStopHandler(handler: () => void): void {
    this.onStop = handler;
  }

  public setShaderUniform(
    shaderName: string,
    uniformName: string,
    value: ShaderUniformValue,
  ): void {
    this.shaderRegistry.setUniform(shaderName, uniformName, value);
  }

  public materialShader(shaderName: string): ShaderMaterial {
    return this.shaderRegistry.material(shaderName);
  }

  public meshBox(width: number, height: number, depth: number): MeshHandle {
    return this.scene.meshBox(width, height, depth);
  }

  public meshSphere(radius: number, segments: number): MeshHandle {
    return this.scene.meshSphere(radius, segments);
  }

  public meshPlane(
    width: number,
    depth: number,
    columns = 1,
    rows = 1,
  ): MeshHandle {
    return this.scene.meshPlane(width, depth, columns, rows);
  }

  public meshFrom(
    vertices: readonly number[],
    indices: readonly number[],
  ): MeshHandle {
    return this.scene.meshFrom(vertices, indices);
  }

  public materialBasic(color: NumericSequence): BasicMaterial {
    return this.scene.materialBasic(color);
  }

  public nodeAdd(mesh: MeshHandle, material: MaterialHandle): NodeHandle {
    return this.scene.nodeAdd(mesh, material);
  }

  public nodeRemove(node: NodeHandle): void {
    this.scene.nodeRemove(node);
  }

  public meshDispose(mesh: MeshHandle): void {
    this.scene.meshDispose(mesh);
  }

  public nodeSetPosition(
    node: NodeHandle,
    x: number,
    y: number,
    z: number,
  ): void {
    this.scene.nodeSetPosition(node, x, y, z);
  }

  public nodeSetRotation(
    node: NodeHandle,
    x: number,
    y: number,
    z: number,
  ): void {
    this.scene.nodeSetRotation(node, x, y, z);
  }

  public nodeSetScale(
    node: NodeHandle,
    x: number,
    y: number,
    z: number,
  ): void {
    this.scene.nodeSetScale(node, x, y, z);
  }

  public cameraPerspective(
    verticalFov: number,
    near: number,
    far: number,
  ): void {
    this.scene.cameraPerspective(verticalFov, near, far);
  }

  public cameraLookAt(
    eye: NumericSequence,
    target: NumericSequence,
    up: NumericSequence,
  ): void {
    this.scene.cameraLookAt(eye, target, up);
  }

  public lightDirectional(
    direction: NumericSequence,
    color: NumericSequence,
  ): void {
    this.scene.lightDirectional(direction, color);
  }

  public textureLoad(path: string): TextureHandle {
    return this.scene.textureLoad(path);
  }

  public textureDispose(texture: TextureHandle): void {
    this.scene.textureDispose(texture);
  }

  public shaderSet(
    node: NodeHandle,
    name: string,
    value: SceneShaderValue,
  ): void {
    this.scene.shaderSet(node, name, value);
  }

  private program: PolyglProgram | undefined;

  private readonly tick = (timestamp: number): void => {
    if (this.stopped || this.sessionState !== "running") {
      return;
    }
    const previous = this.previousTimestamp;
    this.previousTimestamp = timestamp;
    const realDelta = previous === undefined
      ? 0
      : Math.min(
        this.maxDeltaSeconds,
        Math.max(0, (timestamp - previous) / 1000),
      );
    try {
      this.advanceSimulation(realDelta);
      if (this.stopped) return;
      this.render();
      this.animationHandle = this.requestFrame(this.tick);
    } catch (error) {
      this.fail(error);
    }
  };

  private installInputListeners(): void {
    this.canvas.addEventListener("pointermove", this.handlePointerMove);
    this.canvas.addEventListener("pointerdown", this.handlePointerDown);
    this.canvas.addEventListener("pointerup", this.handlePointerUp);
    this.canvas.addEventListener("pointercancel", this.handlePointerCancel);
    this.canvas.addEventListener("wheel", this.handleWheel);
    this.documentObject?.addEventListener("keydown", this.handleKeyDown);
    this.documentObject?.addEventListener("keyup", this.handleKeyUp);
    this.documentObject?.addEventListener(
      "visibilitychange",
      this.handleVisibilityChange,
    );
    this.windowObject?.addEventListener("blur", this.handleBlur);
  }

  private installContextListeners(): void {
    this.canvas.addEventListener("webglcontextlost", this.handleContextLost);
    this.canvas.addEventListener(
      "webglcontextrestored",
      this.handleContextRestored,
    );
  }

  private installResizeObserver(
    createObserver: RuntimeOptions["createResizeObserver"],
  ): void {
    const factory = createObserver ?? defaultResizeObserverFactory();
    if (factory === undefined) {
      throw new Error(
        "autoResize requires ResizeObserver support or createResizeObserver",
      );
    }
    this.resizeObserver = validateResizeObserver(factory(this.handleResize));
    this.resizeObserver.observe(this.canvas);
  }

  private readonly handlePointerMove = (event: PointerEvent): void => {
    if (!this.updatePointerPosition(event)) {
      return;
    }
    this.dispatchPointerEvent("pointermove", event);
  };

  private readonly handlePointerDown = (event: PointerEvent): void => {
    if (!this.updatePointerPosition(event)) {
      return;
    }
    if (this.focusOnPointerDown) this.canvas.focus();
    this.canvas.setPointerCapture?.(event.pointerId);
    this.dispatchPointerEvent("pointerdown", event);
  };

  private readonly handlePointerUp = (event: PointerEvent): void => {
    if (this.updatePointerPosition(event)) {
      this.dispatchPointerEvent("pointerup", event);
    }
    if (this.canvas.hasPointerCapture?.(event.pointerId)) {
      this.canvas.releasePointerCapture(event.pointerId);
    }
  };

  private readonly handlePointerCancel = (event: PointerEvent): void => {
    if (this.updatePointerPosition(event)) {
      this.dispatchPointerEvent("pointercancel", event);
    }
    if (this.canvas.hasPointerCapture?.(event.pointerId)) {
      this.canvas.releasePointerCapture(event.pointerId);
    }
  };

  private updatePointerPosition(event: MouseEvent): boolean {
    const bounds = this.canvas.getBoundingClientRect();
    if (bounds.width <= 0 || bounds.height <= 0) {
      return false;
    }
    this.mouseX = ((event.clientX - bounds.left) / bounds.width) * this.canvas.width;
    this.mouseY = ((event.clientY - bounds.top) / bounds.height) * this.canvas.height;
    return Number.isFinite(this.mouseX) && Number.isFinite(this.mouseY);
  }

  private dispatchPointerEvent(kind: string, event: PointerEvent): void {
    this.preventDefault(event);
    this.dispatchEvent({
      kind,
      x: this.mouseX,
      y: this.mouseY,
      key: null,
      code: null,
      pointerId: finiteIntegerOrNull(event.pointerId),
      buttons: finiteInteger(event.buttons),
      button: finiteInteger(event.button, -1),
      pressure: finiteNumber(event.pressure),
      wheelX: 0,
      wheelY: 0,
      ...modifiers(event),
      inside: this.pointerInside(event),
    });
  }

  private readonly handleWheel = (event: WheelEvent): void => {
    if (!this.updatePointerPosition(event)) return;
    this.preventDefault(event);
    this.dispatchEvent({
      kind: "wheel",
      x: this.mouseX,
      y: this.mouseY,
      key: null,
      code: null,
      pointerId: null,
      buttons: finiteInteger(event.buttons),
      button: -1,
      pressure: 0,
      wheelX: finiteNumber(event.deltaX),
      wheelY: finiteNumber(event.deltaY),
      ...modifiers(event),
      inside: this.pointerInside(event),
    });
  };

  private readonly handleKeyDown = (event: KeyboardEvent): void => {
    this.preventDefault(event);
    const key = stringOrNull(event.key);
    if (key !== null) this.pressedKeys.add(key);
    this.dispatchEvent({
      kind: "keydown",
      x: this.mouseX,
      y: this.mouseY,
      key,
      code: stringOrNull(event.code),
      pointerId: null,
      buttons: 0,
      button: -1,
      pressure: 0,
      wheelX: 0,
      wheelY: 0,
      ...modifiers(event),
      inside: this.pointerWithinCanvas(),
    });
  };

  private readonly handleKeyUp = (event: KeyboardEvent): void => {
    this.preventDefault(event);
    const key = stringOrNull(event.key);
    if (key !== null) this.pressedKeys.delete(key);
    this.dispatchEvent({
      kind: "keyup",
      x: this.mouseX,
      y: this.mouseY,
      key,
      code: stringOrNull(event.code),
      pointerId: null,
      buttons: 0,
      button: -1,
      pressure: 0,
      wheelX: 0,
      wheelY: 0,
      ...modifiers(event),
      inside: this.pointerWithinCanvas(),
    });
  };

  private readonly handleBlur = (): void => {
    this.pressedKeys.clear();
  };

  private readonly handleVisibilityChange = (): void => {
    if (this.stopped || this.visibilityPolicy !== "pause") return;
    if (this.shouldPauseForVisibility()) {
      this.pauseAnimation();
      this.sessionState = "paused";
      return;
    }
    if (this.contextLost || this.sessionState !== "paused") return;
    this.sessionState = "running";
    this.previousTimestamp = undefined;
    this.fixedAccumulator = 0;
    if (this.program?.frame !== undefined) {
      this.animationHandle = this.requestFrame(this.tick);
    } else {
      this.scheduleRender();
    }
  };

  private readonly handleResize = (): void => {
    if (this.stopped || this.contextLost) {
      return;
    }
    try {
      if (this.syncDisplaySize()) {
        this.scheduleRender();
      }
    } catch (error) {
      this.fail(error);
    }
  };

  private readonly handleContextLost = (event: Event): void => {
    event.preventDefault();
    if (this.stopped || this.contextLost) {
      return;
    }
    this.contextLost = true;
    this.sessionState = "context-lost";
    this.pauseAnimation();
    this.onError(
      new Error("WebGL context was lost; rendering is suspended"),
    );
  };

  private readonly handleContextRestored = (): void => {
    if (this.stopped || !this.contextLost) {
      return;
    }
    this.fail(
      new Error(
        "WebGL context was restored, but GPU resources must be recreated; restart the runtime session",
      ),
    );
  };

  private dispatchEvent(event: RuntimeEvent): void {
    try {
      this.program?.on_event?.(event);
      if (this.stopped) {
        return;
      }
      this.scheduleRender();
    } catch (error) {
      this.fail(error);
    }
  }

  private advanceSimulation(realDelta: number): void {
    const fixed = this.fixedDeltaSeconds;
    if (fixed === undefined) {
      this.elapsedSeconds += realDelta;
      this.program?.frame?.(realDelta);
      return;
    }
    this.fixedAccumulator = Math.min(
      this.fixedAccumulator + realDelta,
      fixed * this.maxCatchUpSteps,
    );
    let steps = 0;
    while (
      this.fixedAccumulator + Number.EPSILON >= fixed &&
      steps < this.maxCatchUpSteps
    ) {
      this.elapsedSeconds += fixed;
      this.program?.frame?.(fixed);
      if (this.stopped) return;
      this.fixedAccumulator -= fixed;
      steps += 1;
    }
  }

  private pauseAnimation(): void {
    if (this.animationHandle !== undefined) {
      this.cancelFrame(this.animationHandle);
      this.animationHandle = undefined;
    }
    if (this.renderHandle !== undefined) {
      this.cancelFrame(this.renderHandle);
      this.renderHandle = undefined;
    }
  }

  private shouldPauseForVisibility(): boolean {
    return this.visibilityPolicy === "pause" &&
      this.documentObject?.hidden === true;
  }

  private preventDefault(event: Event): void {
    if (this.preventDefaultInput) event.preventDefault();
  }

  private pointerInside(event: MouseEvent): boolean {
    const bounds = this.canvas.getBoundingClientRect();
    return event.clientX >= bounds.left &&
      event.clientX <= bounds.left + bounds.width &&
      event.clientY >= bounds.top &&
      event.clientY <= bounds.top + bounds.height;
  }

  private pointerWithinCanvas(): boolean {
    return this.mouseX >= 0 &&
      this.mouseX <= this.canvas.width &&
      this.mouseY >= 0 &&
      this.mouseY <= this.canvas.height;
  }

  private scheduleRender(): void {
    if (
      this.stopped ||
      this.contextLost ||
      this.program?.frame !== undefined ||
      this.renderHandle !== undefined
    ) {
      return;
    }
    this.renderHandle = this.requestFrame(() => {
      this.renderHandle = undefined;
      if (this.stopped || this.contextLost) {
        return;
      }
      try {
        this.render();
      } catch (error) {
        this.fail(error);
      }
    });
  }

  private syncDisplaySize(): boolean {
    const bounds = this.canvas.getBoundingClientRect();
    if (bounds.width <= 0 || bounds.height <= 0) {
      return false;
    }
    const ratio = this.configuredDevicePixelRatio ??
      this.windowObject?.devicePixelRatio ?? 1;
    const width = Math.max(1, Math.round(bounds.width * ratio));
    const height = Math.max(1, Math.round(bounds.height * ratio));
    if (this.canvas.width === width && this.canvas.height === height) {
      return false;
    }
    this.renderer.resize(width, height);
    return true;
  }

  private updateShaderUniforms(): void {
    this.shaderRegistry.updateAutomaticUniforms(
      this.elapsedSeconds,
      this.canvas.width,
      this.canvas.height,
    );
  }

  private render(): void {
    this.updateShaderUniforms();
    this.scene.render(
      this.elapsedSeconds,
      this.canvas.width,
      this.canvas.height,
    );
    this.renderer.flush();
  }

  private replaceShaderBundle(
    bundle: ShaderBundle | undefined,
    requireShaderAbi = false,
  ): void {
    this.shaderRegistry.dispose();
    this.shaderRegistry = WebGL2ShaderRegistry.fromBundle(
      this.renderer.context,
      bundle,
      requireShaderAbi,
    );
    this.scene.replaceShaderRegistry(this.shaderRegistry);
  }

  private fail(reason: unknown): void {
    this.stop();
    this.onError(reason);
  }
}

interface FrameScheduler {
  readonly request: (callback: FrameRequestCallback) => number;
  readonly cancel: (handle: number) => void;
}

function resolveFrameScheduler(options: RuntimeOptions): FrameScheduler {
  if (options.requestAnimationFrame !== undefined) {
    return {
      request: options.requestAnimationFrame,
      cancel: options.cancelAnimationFrame ?? defaultFrameCanceller(),
    };
  }
  if (typeof globalThis.requestAnimationFrame === "function") {
    return {
      request: (callback) => globalThis.requestAnimationFrame(callback),
      cancel: options.cancelAnimationFrame ?? defaultFrameCanceller(),
    };
  }
  return {
    request: (callback) => globalThis.setTimeout(
      () => callback(globalThis.performance.now()),
      16,
    ),
    cancel: (handle) => globalThis.clearTimeout(handle),
  };
}

function defaultFrameCanceller(): (handle: number) => void {
  return typeof globalThis.cancelAnimationFrame === "function"
    ? (handle) => globalThis.cancelAnimationFrame(handle)
    : () => {};
}

function defaultResizeObserverFactory(): RuntimeOptions["createResizeObserver"] {
  if (typeof globalThis.ResizeObserver !== "function") {
    return undefined;
  }
  return (callback) => new globalThis.ResizeObserver(() => callback());
}

function positiveFinite(value: number, name: string): number {
  if (!Number.isFinite(value) || value <= 0) {
    throw new RangeError(`${name} must be a finite number greater than zero`);
  }
  return value;
}

function finiteNumber(value: unknown, fallback = 0): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function finiteInteger(value: unknown, fallback = 0): number {
  return typeof value === "number" && Number.isSafeInteger(value)
    ? value
    : fallback;
}

function finiteIntegerOrNull(value: unknown): number | null {
  return typeof value === "number" && Number.isSafeInteger(value)
    ? value
    : null;
}

function stringOrNull(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

function modifiers(event: MouseEvent | KeyboardEvent): Pick<
  RuntimeEvent,
  "altKey" | "ctrlKey" | "metaKey" | "shiftKey"
> {
  return {
    altKey: event.altKey === true,
    ctrlKey: event.ctrlKey === true,
    metaKey: event.metaKey === true,
    shiftKey: event.shiftKey === true,
  };
}
