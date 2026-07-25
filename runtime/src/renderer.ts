type Color = readonly [number, number, number, number];

const FLOATS_PER_VERTEX = 6;
const CIRCLE_SEGMENTS = 32;

export class WebGL2BatchRenderer {
  private readonly gl: WebGL2RenderingContext;
  private readonly program: WebGLProgram;
  private readonly buffer: WebGLBuffer;
  private readonly resolution: WebGLUniformLocation;
  private readonly vertices: number[] = [];
  private fillColor: Color = [1, 1, 1, 1];

  public constructor(
    private readonly canvas: HTMLCanvasElement,
    context?: WebGL2RenderingContext,
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
  }

  public background(r: number, g: number, b: number): void {
    this.flush();
    this.gl.clearColor(channel(r), channel(g), channel(b), 1);
    this.gl.clear(this.gl.COLOR_BUFFER_BIT);
  }

  public fill(r: number, g: number, b: number, a = 1): void {
    this.fillColor = [channel(r), channel(g), channel(b), channel(a)];
  }

  public rect(x: number, y: number, width: number, height: number): void {
    const right = x + width;
    const bottom = y + height;
    this.triangle(x, y, right, y, right, bottom);
    this.triangle(x, y, right, bottom, x, bottom);
  }

  public circle(x: number, y: number, radius: number): void {
    if (!Number.isFinite(radius) || radius < 0) {
      throw new RangeError("circle radius must be a non-negative finite number");
    }
    for (let segment = 0; segment < CIRCLE_SEGMENTS; segment += 1) {
      const start = (segment / CIRCLE_SEGMENTS) * Math.PI * 2;
      const end = ((segment + 1) / CIRCLE_SEGMENTS) * Math.PI * 2;
      this.triangle(
        x,
        y,
        x + Math.cos(start) * radius,
        y + Math.sin(start) * radius,
        x + Math.cos(end) * radius,
        y + Math.sin(end) * radius,
      );
    }
  }

  public triangle(
    x1: number,
    y1: number,
    x2: number,
    y2: number,
    x3: number,
    y3: number,
  ): void {
    this.vertex(x1, y1);
    this.vertex(x2, y2);
    this.vertex(x3, y3);
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
  }

  private vertex(x: number, y: number): void {
    if (!Number.isFinite(x) || !Number.isFinite(y)) {
      throw new RangeError("shape coordinates must be finite numbers");
    }
    this.vertices.push(x, y, ...this.fillColor);
  }
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
