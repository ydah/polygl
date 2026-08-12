type Color = readonly [number, number, number, number];
type Matrix2D = readonly [
  number,
  number,
  number,
  number,
  number,
  number,
];

export type StrokeCap = "butt" | "square" | "round";
export type StrokeJoin = "miter" | "bevel" | "round";

export interface BatchRendererStats {
  readonly drawCalls: number;
  readonly triangles: number;
  readonly uploadedBytes: number;
  readonly bufferGrowths: number;
  readonly bufferCapacityBytes: number;
}

interface ScreenSegment {
  readonly startX: number;
  readonly startY: number;
  readonly endX: number;
  readonly endY: number;
  readonly unitX: number;
  readonly unitY: number;
  readonly normalX: number;
  readonly normalY: number;
}

const FLOATS_PER_VERTEX = 6;
const IDENTITY_TRANSFORM: Matrix2D = [1, 0, 0, 1, 0, 0];
const MAX_FLOAT32 = 3.4028234663852886e38;
const INITIAL_VERTEX_CAPACITY = 256;
const MIN_CIRCLE_SEGMENTS = 8;
const MAX_CIRCLE_SEGMENTS = 512;

export class WebGL2BatchRenderer {
  private readonly gl: WebGL2RenderingContext;
  private readonly program: WebGLProgram;
  private readonly buffer: WebGLBuffer;
  private readonly vertexArray: WebGLVertexArrayObject;
  private readonly positionAttribute: number;
  private readonly colorAttribute: number;
  private readonly resolution: WebGLUniformLocation;
  private vertices = new Float32Array(INITIAL_VERTEX_CAPACITY);
  private vertexFloatCount = 0;
  private bufferCapacityBytes = 0;
  private drawCalls = 0;
  private triangles = 0;
  private uploadedBytes = 0;
  private bufferGrowths = 0;
  private readonly textOverlay: Canvas2DTextOverlay | undefined;
  private fillColor: Color = [1, 1, 1, 1];
  private strokeColor: Color | undefined;
  private strokeWidthValue = 1;
  private strokeCapValue: StrokeCap = "butt";
  private strokeJoinValue: StrokeJoin = "miter";
  private transform: Matrix2D = IDENTITY_TRANSFORM;
  private readonly transformStack: Matrix2D[] = [];

  public constructor(
    private readonly canvas: HTMLCanvasElement,
    context?: WebGL2RenderingContext,
    documentObject?: Document,
  ) {
    const initialWidth = positiveInteger(canvas.width, "canvas width");
    const initialHeight = positiveInteger(canvas.height, "canvas height");
    const gl =
      context ??
      canvas.getContext("webgl2", {
        alpha: true,
        antialias: false,
      });
    if (gl === null) {
      throw new Error("WebGL2 is not available");
    }
    this.gl = gl;
    this.program = createProgram(gl);
    const buffer = gl.createBuffer();
    if (buffer === null) {
      gl.deleteProgram(this.program);
      throw new Error("failed to create the WebGL2 vertex buffer");
    }
    this.buffer = buffer;
    const vertexArray = gl.createVertexArray();
    if (vertexArray === null) {
      gl.deleteBuffer(this.buffer);
      gl.deleteProgram(this.program);
      throw new Error("failed to create the WebGL2 vertex array");
    }
    this.vertexArray = vertexArray;

    const position = gl.getAttribLocation(this.program, "a_position");
    const color = gl.getAttribLocation(this.program, "a_color");
    const resolution = gl.getUniformLocation(this.program, "u_resolution");
    if (position < 0 || color < 0 || resolution === null) {
      gl.deleteVertexArray(this.vertexArray);
      gl.deleteBuffer(this.buffer);
      gl.deleteProgram(this.program);
      throw new Error("the built-in WebGL2 shader interface is incomplete");
    }
    this.positionAttribute = position;
    this.colorAttribute = color;
    this.resolution = resolution;

    gl.useProgram(this.program);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    gl.bindVertexArray(this.vertexArray);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer);
    gl.enableVertexAttribArray(this.positionAttribute);
    gl.vertexAttribPointer(
      this.positionAttribute,
      2,
      gl.FLOAT,
      false,
      FLOATS_PER_VERTEX * Float32Array.BYTES_PER_ELEMENT,
      0,
    );
    gl.enableVertexAttribArray(this.colorAttribute);
    gl.vertexAttribPointer(
      this.colorAttribute,
      4,
      gl.FLOAT,
      false,
      FLOATS_PER_VERTEX * Float32Array.BYTES_PER_ELEMENT,
      2 * Float32Array.BYTES_PER_ELEMENT,
    );
    gl.bindVertexArray(null);
    let overlay: Canvas2DTextOverlay | undefined;
    try {
      overlay = Canvas2DTextOverlay.attach(canvas, documentObject);
      this.textOverlay = overlay;
      this.resize(initialWidth, initialHeight);
    } catch (error) {
      overlay?.dispose();
      gl.deleteVertexArray(this.vertexArray);
      gl.deleteBuffer(this.buffer);
      gl.deleteProgram(this.program);
      throw error;
    }
  }

  public get context(): WebGL2RenderingContext {
    return this.gl;
  }

  public resize(width: number, height: number): void {
    const safeWidth = positiveInteger(width, "canvas width");
    const safeHeight = positiveInteger(height, "canvas height");
    this.flush();
    this.canvas.width = safeWidth;
    this.canvas.height = safeHeight;
    this.gl.viewport(0, 0, safeWidth, safeHeight);
    this.textOverlay?.resize(safeWidth, safeHeight);
  }

  public background(r: number, g: number, b: number): void {
    this.flush();
    this.gl.clearColor(channel(r), channel(g), channel(b), 1);
    this.gl.clear(this.gl.COLOR_BUFFER_BIT | this.gl.DEPTH_BUFFER_BIT);
    this.textOverlay?.clear();
  }

  public fill(r: number, g: number, b: number, a = 1): void {
    this.fillColor = [channel(r), channel(g), channel(b), channel(a)];
  }

  public stroke(r: number, g: number, b: number, a = 1): void {
    this.strokeColor = [channel(r), channel(g), channel(b), channel(a)];
  }

  public noStroke(): void {
    this.strokeColor = undefined;
  }

  public strokeWidth(width: number): void {
    if (!Number.isFinite(width) || width <= 0) {
      throw new RangeError("stroke width must be a finite number greater than zero");
    }
    this.strokeWidthValue = width;
  }

  public strokeCap(cap: StrokeCap): void {
    if (cap !== "butt" && cap !== "square" && cap !== "round") {
      throw new TypeError("stroke cap must be butt, square, or round");
    }
    this.strokeCapValue = cap;
  }

  public strokeJoin(join: StrokeJoin): void {
    if (join !== "miter" && join !== "bevel" && join !== "round") {
      throw new TypeError("stroke join must be miter, bevel, or round");
    }
    this.strokeJoinValue = join;
  }

  public stats(): BatchRendererStats {
    return Object.freeze({
      drawCalls: this.drawCalls,
      triangles: this.triangles,
      uploadedBytes: this.uploadedBytes,
      bufferGrowths: this.bufferGrowths,
      bufferCapacityBytes: this.bufferCapacityBytes,
    });
  }

  public rect(x: number, y: number, width: number, height: number): void {
    const right = x + width;
    const bottom = y + height;
    this.fillTriangle(x, y, right, y, right, bottom);
    this.fillTriangle(x, y, right, bottom, x, bottom);
    if (this.strokeColor !== undefined) {
      this.strokePath(
        [[x, y], [right, y], [right, bottom], [x, bottom]],
        true,
        this.strokeColor,
      );
    }
  }

  public circle(x: number, y: number, radius: number): void {
    if (!Number.isFinite(radius) || radius < 0) {
      throw new RangeError("circle radius must be a non-negative finite number");
    }
    if (radius === 0) return;
    const segments = this.circleSegments(radius);
    const outline: Array<readonly [number, number]> = [];
    for (let segment = 0; segment < segments; segment += 1) {
      const start = (segment / segments) * Math.PI * 2;
      const end = ((segment + 1) / segments) * Math.PI * 2;
      const startX = x + Math.cos(start) * radius;
      const startY = y + Math.sin(start) * radius;
      const endX = x + Math.cos(end) * radius;
      const endY = y + Math.sin(end) * radius;
      this.fillTriangle(
        x,
        y,
        startX,
        startY,
        endX,
        endY,
      );
      outline.push([startX, startY]);
    }
    if (this.strokeColor !== undefined) {
      this.strokePath(outline, true, this.strokeColor);
    }
  }

  public line(x1: number, y1: number, x2: number, y2: number): void {
    this.strokeLine(
      x1,
      y1,
      x2,
      y2,
      this.strokeColor ?? this.fillColor,
    );
  }

  public triangle(
    x1: number,
    y1: number,
    x2: number,
    y2: number,
    x3: number,
    y3: number,
  ): void {
    this.fillTriangle(x1, y1, x2, y2, x3, y3);
    if (this.strokeColor !== undefined) {
      this.strokePath([[x1, y1], [x2, y2], [x3, y3]], true, this.strokeColor);
    }
  }

  public text(value: string, x: number, y: number): void {
    this.flush();
    const overlay = this.textOverlay;
    if (overlay === undefined) {
      throw new Error(
        "text requires an attached browser canvas with Canvas2D support",
      );
    }
    overlay.draw(value, x, y, this.fillColor, this.transform);
  }

  public pushMatrix(): void {
    this.transformStack.push(this.transform);
  }

  public popMatrix(): void {
    const transform = this.transformStack.pop();
    if (transform === undefined) {
      throw new Error("pop_matrix called without a matching push_matrix");
    }
    this.transform = transform;
  }

  public translate(x: number, y: number): void {
    this.transform = finiteMatrix(
      multiply(
        this.transform,
        [1, 0, 0, 1, coordinate(x), coordinate(y)],
      ),
    );
  }

  public rotate(radians: number): void {
    const angle = coordinate(radians);
    const cosine = Math.cos(angle);
    const sine = Math.sin(angle);
    this.transform = finiteMatrix(
      multiply(
        this.transform,
        [cosine, sine, -sine, cosine, 0, 0],
      ),
    );
  }

  public scale(x: number, y: number): void {
    this.transform = finiteMatrix(
      multiply(
        this.transform,
        [coordinate(x), 0, 0, coordinate(y), 0, 0],
      ),
    );
  }

  public flush(): void {
    if (this.vertexFloatCount === 0) {
      return;
    }
    const gl = this.gl;
    gl.disable(gl.DEPTH_TEST);
    gl.bindVertexArray(this.vertexArray);
    gl.useProgram(this.program);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer);
    const byteLength = this.vertexFloatCount * Float32Array.BYTES_PER_ELEMENT;
    if (byteLength > this.bufferCapacityBytes) {
      this.bufferCapacityBytes = geometricCapacity(
        Math.max(Float32Array.BYTES_PER_ELEMENT, this.bufferCapacityBytes),
        byteLength,
      );
      gl.bufferData(gl.ARRAY_BUFFER, this.bufferCapacityBytes, gl.DYNAMIC_DRAW);
      this.bufferGrowths += 1;
    }
    gl.bufferSubData(
      gl.ARRAY_BUFFER,
      0,
      this.vertices.subarray(0, this.vertexFloatCount),
    );
    gl.uniform2f(this.resolution, this.canvas.width, this.canvas.height);
    gl.drawArrays(
      gl.TRIANGLES,
      0,
      this.vertexFloatCount / FLOATS_PER_VERTEX,
    );
    this.drawCalls += 1;
    this.triangles += this.vertexFloatCount / (FLOATS_PER_VERTEX * 3);
    this.uploadedBytes += byteLength;
    this.vertexFloatCount = 0;
    gl.bindVertexArray(null);
  }

  public dispose(): void {
    this.vertexFloatCount = 0;
    this.gl.deleteVertexArray(this.vertexArray);
    this.gl.deleteBuffer(this.buffer);
    this.gl.deleteProgram(this.program);
    this.textOverlay?.dispose();
  }

  private fillTriangle(
    x1: number,
    y1: number,
    x2: number,
    y2: number,
    x3: number,
    y3: number,
  ): void {
    this.vertex(x1, y1, this.fillColor);
    this.vertex(x2, y2, this.fillColor);
    this.vertex(x3, y3, this.fillColor);
  }

  private strokeLine(
    x1: number,
    y1: number,
    x2: number,
    y2: number,
    color: Color,
  ): void {
    this.strokePath([[x1, y1], [x2, y2]], false, color);
  }

  private strokePath(
    points: readonly (readonly [number, number])[],
    closed: boolean,
    color: Color,
  ): void {
    const screenPoints = points.map(([x, y]) => this.transformPoint(x, y));
    const segments: ScreenSegment[] = [];
    const count = closed ? screenPoints.length : screenPoints.length - 1;
    for (let index = 0; index < count; index += 1) {
      const start = screenPoints[index];
      const end = screenPoints[(index + 1) % screenPoints.length];
      if (start === undefined || end === undefined) continue;
      const dx = end[0] - start[0];
      const dy = end[1] - start[1];
      const length = Math.hypot(dx, dy);
      if (length <= Number.EPSILON) continue;
      const unitX = dx / length;
      const unitY = dy / length;
      segments.push({
        startX: start[0],
        startY: start[1],
        endX: end[0],
        endY: end[1],
        unitX,
        unitY,
        normalX: -unitY,
        normalY: unitX,
      });
    }
    if (segments.length === 0) return;
    const halfWidth = this.strokeWidthValue / 2;
    for (let index = 0; index < segments.length; index += 1) {
      const segment = segments[index];
      if (segment === undefined) continue;
      const squareStart = !closed && index === 0 && this.strokeCapValue === "square"
        ? halfWidth
        : 0;
      const squareEnd = !closed && index === segments.length - 1 &&
          this.strokeCapValue === "square"
        ? halfWidth
        : 0;
      this.strokeSegmentQuad(
        segment,
        halfWidth,
        squareStart,
        squareEnd,
        color,
      );
    }
    const joinCount = closed ? segments.length : segments.length - 1;
    for (let index = 0; index < joinCount; index += 1) {
      const previous = segments[index];
      const next = segments[(index + 1) % segments.length];
      if (previous !== undefined && next !== undefined) {
        this.strokeJoinGeometry(previous, next, halfWidth, color);
      }
    }
    if (!closed && this.strokeCapValue === "round") {
      const first = segments[0];
      const last = segments[segments.length - 1];
      if (first !== undefined) this.roundCap(first, true, halfWidth, color);
      if (last !== undefined) this.roundCap(last, false, halfWidth, color);
    }
  }

  private strokeSegmentQuad(
    segment: ScreenSegment,
    halfWidth: number,
    extendStart: number,
    extendEnd: number,
    color: Color,
  ): void {
    const startX = segment.startX - segment.unitX * extendStart;
    const startY = segment.startY - segment.unitY * extendStart;
    const endX = segment.endX + segment.unitX * extendEnd;
    const endY = segment.endY + segment.unitY * extendEnd;
    const offsetX = segment.normalX * halfWidth;
    const offsetY = segment.normalY * halfWidth;
    this.screenTriangle(
      [startX + offsetX, startY + offsetY],
      [endX + offsetX, endY + offsetY],
      [endX - offsetX, endY - offsetY],
      color,
    );
    this.screenTriangle(
      [startX + offsetX, startY + offsetY],
      [endX - offsetX, endY - offsetY],
      [startX - offsetX, startY - offsetY],
      color,
    );
  }

  private strokeJoinGeometry(
    previous: ScreenSegment,
    next: ScreenSegment,
    halfWidth: number,
    color: Color,
  ): void {
    const cross = previous.unitX * next.unitY - previous.unitY * next.unitX;
    if (Math.abs(cross) <= 1e-12) return;
    const side = cross > 0 ? -1 : 1;
    const center: readonly [number, number] = [previous.endX, previous.endY];
    const first: readonly [number, number] = [
      center[0] + previous.normalX * halfWidth * side,
      center[1] + previous.normalY * halfWidth * side,
    ];
    const second: readonly [number, number] = [
      center[0] + next.normalX * halfWidth * side,
      center[1] + next.normalY * halfWidth * side,
    ];
    if (this.strokeJoinValue === "bevel") {
      this.screenTriangle(center, first, second, color);
      return;
    }
    if (this.strokeJoinValue === "round") {
      this.roundJoin(center, first, second, cross > 0, color);
      return;
    }
    const firstNormalX = previous.normalX * side;
    const firstNormalY = previous.normalY * side;
    const secondNormalX = next.normalX * side;
    const secondNormalY = next.normalY * side;
    const sumX = firstNormalX + secondNormalX;
    const sumY = firstNormalY + secondNormalY;
    const sumLength = Math.hypot(sumX, sumY);
    if (sumLength <= 1e-12) {
      this.screenTriangle(center, first, second, color);
      return;
    }
    const miterX = sumX / sumLength;
    const miterY = sumY / sumLength;
    const denominator = miterX * secondNormalX + miterY * secondNormalY;
    const miterLength = denominator <= 1e-12 ? Infinity : halfWidth / denominator;
    if (!Number.isFinite(miterLength) || miterLength > halfWidth * 4) {
      this.screenTriangle(center, first, second, color);
      return;
    }
    const tip: readonly [number, number] = [
      center[0] + miterX * miterLength,
      center[1] + miterY * miterLength,
    ];
    this.screenTriangle(first, tip, second, color);
  }

  private roundJoin(
    center: readonly [number, number],
    first: readonly [number, number],
    second: readonly [number, number],
    counterClockwise: boolean,
    color: Color,
  ): void {
    let start = Math.atan2(first[1] - center[1], first[0] - center[0]);
    let end = Math.atan2(second[1] - center[1], second[0] - center[0]);
    if (counterClockwise) {
      while (end < start) end += Math.PI * 2;
    } else {
      while (end > start) end -= Math.PI * 2;
    }
    const steps = Math.max(2, Math.ceil(Math.abs(end - start) * 4));
    for (let index = 0; index < steps; index += 1) {
      const angle1 = start + ((end - start) * index) / steps;
      const angle2 = start + ((end - start) * (index + 1)) / steps;
      const radius = this.strokeWidthValue / 2;
      this.screenTriangle(
        center,
        [center[0] + Math.cos(angle1) * radius, center[1] + Math.sin(angle1) * radius],
        [center[0] + Math.cos(angle2) * radius, center[1] + Math.sin(angle2) * radius],
        color,
      );
    }
  }

  private roundCap(
    segment: ScreenSegment,
    atStart: boolean,
    radius: number,
    color: Color,
  ): void {
    const center: readonly [number, number] = atStart
      ? [segment.startX, segment.startY]
      : [segment.endX, segment.endY];
    const direction = Math.atan2(segment.unitY, segment.unitX);
    const start = atStart ? direction + Math.PI / 2 : direction - Math.PI / 2;
    const steps = Math.max(4, Math.ceil(Math.PI * radius / 2));
    for (let index = 0; index < steps; index += 1) {
      const angle1 = start + (Math.PI * index) / steps;
      const angle2 = start + (Math.PI * (index + 1)) / steps;
      this.screenTriangle(
        center,
        [center[0] + Math.cos(angle1) * radius, center[1] + Math.sin(angle1) * radius],
        [center[0] + Math.cos(angle2) * radius, center[1] + Math.sin(angle2) * radius],
        color,
      );
    }
  }

  private screenTriangle(
    first: readonly [number, number],
    second: readonly [number, number],
    third: readonly [number, number],
    color: Color,
  ): void {
    this.screenVertex(first[0], first[1], color);
    this.screenVertex(second[0], second[1], color);
    this.screenVertex(third[0], third[1], color);
  }

  private vertex(x: number, y: number, color: Color): void {
    const [screenX, screenY] = this.transformPoint(x, y);
    this.screenVertex(screenX, screenY, color);
  }

  private transformPoint(x: number, y: number): readonly [number, number] {
    const safeX = coordinate(x);
    const safeY = coordinate(y);
    const [a, b, c, d, e, f] = this.transform;
    return [
      drawableCoordinate(a * safeX + c * safeY + e),
      drawableCoordinate(b * safeX + d * safeY + f),
    ];
  }

  private screenVertex(x: number, y: number, color: Color): void {
    this.ensureVertexCapacity(FLOATS_PER_VERTEX);
    const offset = this.vertexFloatCount;
    this.vertices[offset] = drawableCoordinate(x);
    this.vertices[offset + 1] = drawableCoordinate(y);
    this.vertices.set(color, offset + 2);
    this.vertexFloatCount += FLOATS_PER_VERTEX;
  }

  private ensureVertexCapacity(additional: number): void {
    const required = this.vertexFloatCount + additional;
    if (required <= this.vertices.length) return;
    const replacement = new Float32Array(
      geometricCapacity(this.vertices.length, required),
    );
    replacement.set(this.vertices);
    this.vertices = replacement;
  }

  private circleSegments(radius: number): number {
    const [a, b, c, d] = this.transform;
    const projectedRadius = radius * Math.max(Math.hypot(a, b), Math.hypot(c, d));
    if (projectedRadius <= 0.5) return MIN_CIRCLE_SEGMENTS;
    const maximumAngle = 2 * Math.acos(
      Math.max(-1, 1 - 0.5 / projectedRadius),
    );
    return Math.min(
      MAX_CIRCLE_SEGMENTS,
      Math.max(MIN_CIRCLE_SEGMENTS, Math.ceil((Math.PI * 2) / maximumAngle)),
    );
  }
}

class Canvas2DTextOverlay {
  private constructor(
    private readonly canvas: HTMLCanvasElement,
    private readonly context: CanvasRenderingContext2D,
    private readonly target: HTMLCanvasElement,
    private readonly wrapper: HTMLDivElement,
    private readonly originalDisplay: string,
    private readonly originalGridArea: string,
  ) {}

  public static attach(
    target: HTMLCanvasElement,
    documentObject: Document | undefined,
  ): Canvas2DTextOverlay | undefined {
    const parent = target.parentElement;
    if (
      documentObject === undefined ||
      parent === null ||
      parent === undefined
    ) {
      return undefined;
    }
    const canvas = documentObject.createElement("canvas");
    const context = canvas.getContext("2d");
    if (context === null) {
      return undefined;
    }
    const wrapper = documentObject.createElement("div");
    const originalDisplay = target.style.display;
    const originalGridArea = target.style.gridArea;
    canvas.id = target.id === "" ? "polygl-text-overlay" : `${target.id}-text`;
    canvas.setAttribute("aria-hidden", "true");
    Object.assign(wrapper.style, {
      display: "inline-block",
      lineHeight: "0",
      position: "relative",
    });
    Object.assign(target.style, {
      display: "block",
      gridArea: "1 / 1",
    });
    Object.assign(canvas.style, {
      inset: "0",
      height: "100%",
      pointerEvents: "none",
      position: "absolute",
      width: "100%",
    });
    parent.insertBefore(wrapper, target);
    wrapper.append(target, canvas);
    return new Canvas2DTextOverlay(
      canvas,
      context,
      target,
      wrapper,
      originalDisplay,
      originalGridArea,
    );
  }

  public resize(width: number, height: number): void {
    this.canvas.width = width;
    this.canvas.height = height;
  }

  public clear(): void {
    this.context.clearRect(0, 0, this.canvas.width, this.canvas.height);
  }

  public draw(
    value: string,
    x: number,
    y: number,
    color: Color,
    transform: Matrix2D,
  ): void {
    const [a, b, c, d, e, f] = transform;
    this.context.save();
    this.context.setTransform(a, b, c, d, e, f);
    this.context.fillStyle = `rgba(${color[0] * 255}, ${color[1] * 255}, ${color[2] * 255}, ${color[3]})`;
    this.context.textBaseline = "alphabetic";
    this.context.fillText(value, coordinate(x), coordinate(y));
    this.context.restore();
  }

  public dispose(): void {
    this.target.style.display = this.originalDisplay;
    this.target.style.gridArea = this.originalGridArea;
    this.wrapper.replaceWith(this.target);
  }
}

function multiply(left: Matrix2D, right: Matrix2D): Matrix2D {
  const [a, b, c, d, e, f] = left;
  const [g, h, i, j, k, l] = right;
  return [
    a * g + c * h,
    b * g + d * h,
    a * i + c * j,
    b * i + d * j,
    a * k + c * l + e,
    b * k + d * l + f,
  ];
}

function coordinate(value: number): number {
  if (!Number.isFinite(value)) {
    throw new RangeError("shape coordinates must be finite numbers");
  }
  return value;
}

function drawableCoordinate(value: number): number {
  if (!Number.isFinite(value) || Math.abs(value) > MAX_FLOAT32) {
    throw new RangeError("transformed coordinates must fit in a finite float");
  }
  return value;
}

function finiteMatrix(matrix: Matrix2D): Matrix2D {
  if (matrix.some((value) => !Number.isFinite(value))) {
    throw new RangeError("the current transform must remain finite");
  }
  return matrix;
}

function createProgram(gl: WebGL2RenderingContext): WebGLProgram {
  const vertex = compileShader(
    gl,
    gl.VERTEX_SHADER,
    `#version 300 es
in vec2 a_position;
in vec4 a_color;
uniform vec2 u_resolution;
out vec4 v_color;

void main() {
  vec2 clip = (a_position / u_resolution) * 2.0 - 1.0;
  gl_Position = vec4(clip * vec2(1.0, -1.0), 0.0, 1.0);
  v_color = a_color;
}`,
  );
  let fragment: WebGLShader | undefined;
  let program: WebGLProgram | undefined;
  try {
    fragment = compileShader(
      gl,
      gl.FRAGMENT_SHADER,
      `#version 300 es
precision highp float;
in vec4 v_color;
out vec4 out_color;

void main() {
  out_color = v_color;
}`,
    );
    program = gl.createProgram() ?? undefined;
    if (program === undefined) {
      throw new Error("failed to create the built-in WebGL2 program");
    }
    gl.attachShader(program, vertex);
    gl.attachShader(program, fragment);
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
      const message = gl.getProgramInfoLog(program) ?? "unknown link failure";
      throw new Error(`failed to link the built-in WebGL2 program: ${message}`);
    }
    return program;
  } catch (error) {
    if (program !== undefined) {
      gl.deleteProgram(program);
    }
    throw error;
  } finally {
    gl.deleteShader(vertex);
    if (fragment !== undefined) {
      gl.deleteShader(fragment);
    }
  }
}

function compileShader(
  gl: WebGL2RenderingContext,
  kind: number,
  source: string,
): WebGLShader {
  const shader = gl.createShader(kind);
  if (shader === null) {
    throw new Error("failed to create a built-in WebGL2 shader");
  }
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    const message = gl.getShaderInfoLog(shader) ?? "unknown compilation failure";
    gl.deleteShader(shader);
    throw new Error(`failed to compile a built-in WebGL2 shader: ${message}`);
  }
  return shader;
}

function channel(value: number): number {
  if (!Number.isFinite(value)) {
    throw new RangeError("color channels must be finite numbers");
  }
  return Math.min(1, Math.max(0, value));
}

function positiveInteger(value: number, label: string): number {
  if (!Number.isSafeInteger(value) || value <= 0) {
    throw new RangeError(`${label} must be a positive safe integer`);
  }
  return value;
}

function geometricCapacity(current: number, required: number): number {
  let capacity = Math.max(1, current);
  while (capacity < required) {
    capacity *= 2;
    if (!Number.isSafeInteger(capacity)) {
      throw new RangeError("renderer vertex capacity exceeds the safe integer range");
    }
  }
  return capacity;
}
