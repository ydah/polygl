type Color = readonly [number, number, number, number];
type Matrix2D = readonly [
  number,
  number,
  number,
  number,
  number,
  number,
];

const FLOATS_PER_VERTEX = 6;
const CIRCLE_SEGMENTS = 32;
const IDENTITY_TRANSFORM: Matrix2D = [1, 0, 0, 1, 0, 0];
const STROKE_WIDTH = 1;

export class WebGL2BatchRenderer {
  private readonly gl: WebGL2RenderingContext;
  private readonly program: WebGLProgram;
  private readonly buffer: WebGLBuffer;
  private readonly resolution: WebGLUniformLocation;
  private readonly vertices: number[] = [];
  private readonly textOverlay: Canvas2DTextOverlay | undefined;
  private fillColor: Color = [1, 1, 1, 1];
  private strokeColor: Color | undefined;
  private transform: Matrix2D = IDENTITY_TRANSFORM;
  private readonly transformStack: Matrix2D[] = [];

  public constructor(
    private readonly canvas: HTMLCanvasElement,
    context?: WebGL2RenderingContext,
    documentObject?: Document,
  ) {
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

    const position = gl.getAttribLocation(this.program, "a_position");
    const color = gl.getAttribLocation(this.program, "a_color");
    const resolution = gl.getUniformLocation(this.program, "u_resolution");
    if (position < 0 || color < 0 || resolution === null) {
      gl.deleteBuffer(this.buffer);
      gl.deleteProgram(this.program);
      throw new Error("the built-in WebGL2 shader interface is incomplete");
    }
    this.resolution = resolution;

    gl.useProgram(this.program);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer);
    gl.enableVertexAttribArray(position);
    gl.vertexAttribPointer(
      position,
      2,
      gl.FLOAT,
      false,
      FLOATS_PER_VERTEX * Float32Array.BYTES_PER_ELEMENT,
      0,
    );
    gl.enableVertexAttribArray(color);
    gl.vertexAttribPointer(
      color,
      4,
      gl.FLOAT,
      false,
      FLOATS_PER_VERTEX * Float32Array.BYTES_PER_ELEMENT,
      2 * Float32Array.BYTES_PER_ELEMENT,
    );
    this.textOverlay = Canvas2DTextOverlay.attach(canvas, documentObject);
    this.resize(canvas.width, canvas.height);
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
    this.gl.clear(this.gl.COLOR_BUFFER_BIT);
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

  public rect(x: number, y: number, width: number, height: number): void {
    const right = x + width;
    const bottom = y + height;
    this.fillTriangle(x, y, right, y, right, bottom);
    this.fillTriangle(x, y, right, bottom, x, bottom);
    if (this.strokeColor !== undefined) {
      this.strokeLine(x, y, right, y, this.strokeColor);
      this.strokeLine(right, y, right, bottom, this.strokeColor);
      this.strokeLine(right, bottom, x, bottom, this.strokeColor);
      this.strokeLine(x, bottom, x, y, this.strokeColor);
    }
  }

  public circle(x: number, y: number, radius: number): void {
    if (!Number.isFinite(radius) || radius < 0) {
      throw new RangeError("circle radius must be a non-negative finite number");
    }
    for (let segment = 0; segment < CIRCLE_SEGMENTS; segment += 1) {
      const start = (segment / CIRCLE_SEGMENTS) * Math.PI * 2;
      const end = ((segment + 1) / CIRCLE_SEGMENTS) * Math.PI * 2;
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
      if (this.strokeColor !== undefined) {
        this.strokeLine(startX, startY, endX, endY, this.strokeColor);
      }
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
      this.strokeLine(x1, y1, x2, y2, this.strokeColor);
      this.strokeLine(x2, y2, x3, y3, this.strokeColor);
      this.strokeLine(x3, y3, x1, y1, this.strokeColor);
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
    this.transform = multiply(
      this.transform,
      [1, 0, 0, 1, coordinate(x), coordinate(y)],
    );
  }

  public rotate(radians: number): void {
    const angle = coordinate(radians);
    const cosine = Math.cos(angle);
    const sine = Math.sin(angle);
    this.transform = multiply(
      this.transform,
      [cosine, sine, -sine, cosine, 0, 0],
    );
  }

  public scale(x: number, y: number): void {
    this.transform = multiply(
      this.transform,
      [coordinate(x), 0, 0, coordinate(y), 0, 0],
    );
  }

  public flush(): void {
    if (this.vertices.length === 0) {
      return;
    }
    const gl = this.gl;
    gl.useProgram(this.program);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer);
    gl.bufferData(
      gl.ARRAY_BUFFER,
      new Float32Array(this.vertices),
      gl.DYNAMIC_DRAW,
    );
    gl.uniform2f(this.resolution, this.canvas.width, this.canvas.height);
    gl.drawArrays(
      gl.TRIANGLES,
      0,
      this.vertices.length / FLOATS_PER_VERTEX,
    );
    this.vertices.length = 0;
  }

  public dispose(): void {
    this.vertices.length = 0;
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
    const startX = coordinate(x1);
    const startY = coordinate(y1);
    const endX = coordinate(x2);
    const endY = coordinate(y2);
    const dx = endX - startX;
    const dy = endY - startY;
    const length = Math.hypot(dx, dy);
    if (length === 0) {
      return;
    }
    const offsetX = (-dy / length) * (STROKE_WIDTH / 2);
    const offsetY = (dx / length) * (STROKE_WIDTH / 2);
    this.vertex(startX + offsetX, startY + offsetY, color);
    this.vertex(endX + offsetX, endY + offsetY, color);
    this.vertex(endX - offsetX, endY - offsetY, color);
    this.vertex(startX + offsetX, startY + offsetY, color);
    this.vertex(endX - offsetX, endY - offsetY, color);
    this.vertex(startX - offsetX, startY - offsetY, color);
  }

  private vertex(x: number, y: number, color: Color): void {
    const safeX = coordinate(x);
    const safeY = coordinate(y);
    const [a, b, c, d, e, f] = this.transform;
    this.vertices.push(
      a * safeX + c * safeY + e,
      b * safeX + d * safeY + f,
      ...color,
    );
  }
}

class Canvas2DTextOverlay {
  private constructor(
    private readonly canvas: HTMLCanvasElement,
    private readonly context: CanvasRenderingContext2D,
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
    canvas.id = target.id === "" ? "polygl-text-overlay" : `${target.id}-text`;
    canvas.setAttribute("aria-hidden", "true");
    Object.assign(target.style, { gridArea: "1 / 1" });
    Object.assign(canvas.style, {
      gridArea: "1 / 1",
      pointerEvents: "none",
    });
    parent.append(canvas);
    return new Canvas2DTextOverlay(canvas, context);
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
    this.canvas.remove();
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
  if (!Number.isInteger(value) || value <= 0) {
    throw new RangeError(`${label} must be a positive integer`);
  }
  return value;
}
