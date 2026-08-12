export interface WebGLStateStats {
  readonly stateChanges: number;
  readonly programSwitches: number;
}

export class WebGLStateCache {
  private program: WebGLProgram | null | undefined;
  private arrayBuffer: WebGLBuffer | null | undefined;
  private elementArrayBuffer: WebGLBuffer | null | undefined;
  private vertexArray: WebGLVertexArrayObject | null | undefined;
  private blendEnabled: boolean | undefined;
  private depthEnabled: boolean | undefined;
  private blendAlphaConfigured = false;
  private depthFunction: number | undefined;
  private activeTextureUnit: number | undefined;
  private readonly textures2d = new Map<number, WebGLTexture | null>();
  private changeCount = 0;
  private switchCount = 0;

  public constructor(private readonly gl: WebGL2RenderingContext) {}

  public useProgram(program: WebGLProgram | null): void {
    if (this.program === program) return;
    this.gl.useProgram(program);
    this.program = program;
    this.changeCount += 1;
    this.switchCount += 1;
  }

  public bindArrayBuffer(buffer: WebGLBuffer | null): void {
    if (this.arrayBuffer === buffer) return;
    this.gl.bindBuffer(this.gl.ARRAY_BUFFER, buffer);
    this.arrayBuffer = buffer;
    this.changeCount += 1;
  }

  public bindElementArrayBuffer(buffer: WebGLBuffer | null): void {
    if (this.elementArrayBuffer === buffer) return;
    this.gl.bindBuffer(this.gl.ELEMENT_ARRAY_BUFFER, buffer);
    this.elementArrayBuffer = buffer;
    this.changeCount += 1;
  }

  public bindVertexArray(vertexArray: WebGLVertexArrayObject | null): void {
    if (this.vertexArray === vertexArray) return;
    this.gl.bindVertexArray(vertexArray);
    this.vertexArray = vertexArray;
    this.changeCount += 1;
    this.elementArrayBuffer = undefined;
  }

  public enableBlend(): void {
    if (this.blendEnabled !== true) {
      this.gl.enable(this.gl.BLEND);
      this.blendEnabled = true;
      this.changeCount += 1;
    }
    if (!this.blendAlphaConfigured) {
      this.gl.blendFunc(this.gl.SRC_ALPHA, this.gl.ONE_MINUS_SRC_ALPHA);
      this.blendAlphaConfigured = true;
      this.changeCount += 1;
    }
  }

  public setDepthTest(enabled: boolean): void {
    if (this.depthEnabled === enabled) return;
    if (enabled) this.gl.enable(this.gl.DEPTH_TEST);
    else this.gl.disable(this.gl.DEPTH_TEST);
    this.depthEnabled = enabled;
    this.changeCount += 1;
  }

  public setDepthFunction(value: number): void {
    if (this.depthFunction === value) return;
    this.gl.depthFunc(value);
    this.depthFunction = value;
    this.changeCount += 1;
  }

  public activateTexture(unit: number): void {
    if (this.activeTextureUnit === unit) return;
    this.gl.activeTexture(unit);
    this.activeTextureUnit = unit;
    this.changeCount += 1;
  }

  public bindTexture2d(texture: WebGLTexture | null): void {
    const unit = this.activeTextureUnit ?? this.gl.TEXTURE0;
    if (this.activeTextureUnit === undefined) this.activateTexture(unit);
    if (this.textures2d.get(unit) === texture) return;
    this.gl.bindTexture(this.gl.TEXTURE_2D, texture);
    this.textures2d.set(unit, texture);
    this.changeCount += 1;
  }

  public invalidate(): void {
    this.program = undefined;
    this.arrayBuffer = undefined;
    this.elementArrayBuffer = undefined;
    this.vertexArray = undefined;
    this.blendEnabled = undefined;
    this.depthEnabled = undefined;
    this.blendAlphaConfigured = false;
    this.depthFunction = undefined;
    this.activeTextureUnit = undefined;
    this.textures2d.clear();
  }

  public stats(): WebGLStateStats {
    return Object.freeze({
      stateChanges: this.changeCount,
      programSwitches: this.switchCount,
    });
  }
}
