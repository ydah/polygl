import { showRuntimeError } from "./errors.js";
import { SeededRandom } from "./random.js";
import { WebGL2BatchRenderer } from "./renderer.js";

export interface RuntimeEvent {
  readonly kind: string;
  readonly x: number;
  readonly y: number;
  readonly key: string | null;
}

export interface PolyglProgram {
  readonly setup?: () => void | Promise<void>;
  readonly frame?: (dt: number) => void;
  readonly on_event?: (event: RuntimeEvent) => void;
}

export type PolyglProgramLoader = () => Promise<PolyglProgram>;
export type PolyglProgramSource = PolyglProgram | PolyglProgramLoader;

export interface RuntimeOptions {
  readonly canvas?: HTMLCanvasElement;
  readonly context?: WebGL2RenderingContext;
  readonly document?: Document;
  readonly requestAnimationFrame?: (callback: FrameRequestCallback) => number;
  readonly cancelAnimationFrame?: (handle: number) => void;
  readonly seed?: number;
  readonly onError?: (reason: unknown) => void;
}

export interface RuntimeHandle {
  readonly canvas: HTMLCanvasElement;
  stop(): void;
}

export class RuntimeSession implements RuntimeHandle {
  public readonly renderer: WebGL2BatchRenderer;
  public readonly randomSource: SeededRandom;
  public mouseX = 0;
  public mouseY = 0;
  public elapsedSeconds = 0;
  private readonly pressedKeys = new Set<string>();
  private readonly documentObject: Document | undefined;
  private readonly requestFrame: (callback: FrameRequestCallback) => number;
  private readonly cancelFrame: (handle: number) => void;
  private readonly onError: (reason: unknown) => void;
  private animationHandle: number | undefined;
  private previousTimestamp: number | undefined;
  private stopped = false;
  private onStop: () => void = () => {};

  public constructor(
    public readonly canvas: HTMLCanvasElement,
    options: RuntimeOptions,
  ) {
    this.renderer = new WebGL2BatchRenderer(canvas, options.context);
    this.randomSource = new SeededRandom(options.seed);
    this.documentObject = options.document ?? globalThis.document;
    this.requestFrame =
      options.requestAnimationFrame ??
      ((callback) => globalThis.requestAnimationFrame(callback));
    this.cancelFrame =
      options.cancelAnimationFrame ??
      ((handle) => globalThis.cancelAnimationFrame(handle));
    this.onError =
      options.onError ??
      ((reason) => showRuntimeError(reason, this.documentObject));
    this.installInputListeners();
  }

  public async run(source: PolyglProgramSource): Promise<void> {
    try {
      const program = typeof source === "function" ? await source() : source;
      if (this.stopped) {
        return;
      }
      this.program = program;
      await program.setup?.();
      if (this.stopped) {
        return;
      }
      this.renderer.flush();
      if (program.frame !== undefined) {
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
    if (this.animationHandle !== undefined) {
      this.cancelFrame(this.animationHandle);
      this.animationHandle = undefined;
    }
    this.canvas.removeEventListener("pointermove", this.handlePointerMove);
    this.documentObject?.removeEventListener("keydown", this.handleKeyDown);
    this.documentObject?.removeEventListener("keyup", this.handleKeyUp);
    this.renderer.dispose();
    this.onStop();
  }

  public keyDown(key: string): boolean {
    return this.pressedKeys.has(key);
  }

  public setStopHandler(handler: () => void): void {
    this.onStop = handler;
  }

  private program: PolyglProgram | undefined;

  private readonly tick = (timestamp: number): void => {
    if (this.stopped) {
      return;
    }
    const previous = this.previousTimestamp;
    this.previousTimestamp = timestamp;
    const dt =
      previous === undefined ? 0 : Math.max(0, (timestamp - previous) / 1000);
    this.elapsedSeconds += dt;
    try {
      this.program?.frame?.(dt);
      if (this.stopped) {
        return;
      }
      this.renderer.flush();
      this.animationHandle = this.requestFrame(this.tick);
    } catch (error) {
      this.fail(error);
    }
  };

  private installInputListeners(): void {
    this.canvas.addEventListener("pointermove", this.handlePointerMove);
    this.documentObject?.addEventListener("keydown", this.handleKeyDown);
    this.documentObject?.addEventListener("keyup", this.handleKeyUp);
  }

  private readonly handlePointerMove = (event: PointerEvent): void => {
    const bounds = this.canvas.getBoundingClientRect();
    this.mouseX =
      ((event.clientX - bounds.left) / bounds.width) * this.canvas.width;
    this.mouseY =
      ((event.clientY - bounds.top) / bounds.height) * this.canvas.height;
    this.dispatchEvent({
      kind: "pointermove",
      x: this.mouseX,
      y: this.mouseY,
      key: null,
    });
  };

  private readonly handleKeyDown = (event: KeyboardEvent): void => {
    this.pressedKeys.add(event.key);
    this.dispatchEvent({
      kind: "keydown",
      x: this.mouseX,
      y: this.mouseY,
      key: event.key,
    });
  };

  private readonly handleKeyUp = (event: KeyboardEvent): void => {
    this.pressedKeys.delete(event.key);
    this.dispatchEvent({
      kind: "keyup",
      x: this.mouseX,
      y: this.mouseY,
      key: event.key,
    });
  };

  private dispatchEvent(event: RuntimeEvent): void {
    try {
      this.program?.on_event?.(event);
      if (this.stopped) {
        return;
      }
      this.renderer.flush();
    } catch (error) {
      this.fail(error);
    }
  }

  private fail(reason: unknown): void {
    this.stop();
    this.onError(reason);
  }
}
