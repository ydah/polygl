import { runtimeError } from "./errors.js";
import type { SourceLocation } from "./errors.js";

export type ShaderValueType =
  | "int"
  | "float"
  | "bool"
  | "vec2"
  | "vec3"
  | "vec4"
  | "mat2"
  | "mat3"
  | "mat4"
  | "texture";

export interface ShaderAttribute {
  readonly name: string;
  readonly glslName: string;
  readonly location: number;
  readonly type: ShaderValueType;
}

export interface ShaderUniform {
  readonly name: string;
  readonly glslName: string;
  readonly type: ShaderValueType;
  readonly source: "automatic" | "user";
}

export interface ShaderArtifact {
  readonly name: string;
  readonly vertex: string;
  readonly fragment: string;
  readonly attributes: readonly ShaderAttribute[];
  readonly uniforms: readonly ShaderUniform[];
  readonly vertexLocation: SourceLocation;
  readonly fragmentLocation: SourceLocation;
}

export interface ShaderBundle {
  readonly debug: boolean;
  readonly shaders: readonly ShaderArtifact[];
}

export type NumericSequence = readonly number[] | Float32Array;

export type ShaderUniformValue =
  | number
  | boolean
  | NumericSequence
  | WebGLTexture;

export interface ShaderAutomaticUniforms {
  readonly elapsedSeconds: number;
  readonly width: number;
  readonly height: number;
  readonly model: Float32Array;
  readonly view: Float32Array;
  readonly projection: Float32Array;
}

const shaderMaterialBrand: unique symbol = Symbol("ShaderMaterial");

export interface ShaderMaterial {
  readonly kind: "shader";
  readonly shaderName: string;
  readonly [shaderMaterialBrand]: true;
}

interface LinkedShader {
  readonly artifact: ShaderArtifact;
  readonly material: ShaderMaterial;
  readonly program: WebGLProgram;
  readonly uniforms: ReadonlyMap<string, WebGLUniformLocation>;
  readonly userValues: Map<string, ShaderUniformValue>;
  globalUniformsSet: boolean;
}

const IDENTITY_MATRIX = new Float32Array([
  1, 0, 0, 0,
  0, 1, 0, 0,
  0, 0, 1, 0,
  0, 0, 0, 1,
]);

export class WebGL2ShaderRegistry {
  private readonly shaders = new Map<string, LinkedShader>();

  public constructor(
    private readonly gl: WebGL2RenderingContext,
    private readonly debug: boolean,
    artifacts: readonly ShaderArtifact[],
  ) {
    try {
      for (const artifact of artifacts) {
        if (this.shaders.has(artifact.name)) {
          throw runtimeError(
            `shader pair \`${artifact.name}\` is registered more than once`,
            artifact.vertexLocation,
          );
        }
        this.shaders.set(artifact.name, this.link(artifact));
      }
    } catch (error) {
      this.dispose();
      throw error;
    }
  }

  public static fromBundle(
    gl: WebGL2RenderingContext,
    bundle?: ShaderBundle,
  ): WebGL2ShaderRegistry {
    return new WebGL2ShaderRegistry(
      gl,
      bundle?.debug ?? false,
      bundle?.shaders ?? [],
    );
  }

  public setUniform(
    shaderName: string,
    uniformName: string,
    value: ShaderUniformValue,
  ): void {
    const shader = this.shaders.get(shaderName);
    if (shader === undefined) {
      throw new Error(`unknown shader pair \`${shaderName}\``);
    }
    const binding = shader.artifact.uniforms.find(
      (uniform) => uniform.name === uniformName,
    );
    if (binding === undefined || binding.source !== "user") {
      throw runtimeError(
        `shader \`${shaderName}\` has no user uniform \`${uniformName}\``,
        shader.artifact.fragmentLocation,
      );
    }
    validateUniformValue(
      this.gl,
      binding,
      value,
      shader.artifact.fragmentLocation,
    );
    shader.userValues.set(
      uniformName,
      copyUniformValue(value),
    );
    shader.globalUniformsSet = true;
  }

  public material(shaderName: string): ShaderMaterial {
    const shader = this.shaders.get(shaderName);
    if (shader === undefined) {
      throw new Error(`unknown shader pair \`${shaderName}\``);
    }
    return shader.material;
  }

  public owns(material: ShaderMaterial): boolean {
    return this.shaders.get(material.shaderName)?.material === material;
  }

  public nodeUniform(
    material: ShaderMaterial,
    uniformName: string,
    value: ShaderUniformValue,
  ): ShaderUniformValue {
    const shader = this.requireMaterial(material);
    const binding = shader.artifact.uniforms.find(
      (uniform) => uniform.name === uniformName,
    );
    if (binding === undefined || binding.source !== "user") {
      throw runtimeError(
        `shader \`${shader.artifact.name}\` has no user uniform \`${uniformName}\``,
        shader.artifact.fragmentLocation,
      );
    }
    validateUniformValue(
      this.gl,
      binding,
      value,
      shader.artifact.fragmentLocation,
    );
    return copyUniformValue(value);
  }

  public bindForDraw(
    material: ShaderMaterial,
    userValues: ReadonlyMap<string, ShaderUniformValue>,
    automatic: ShaderAutomaticUniforms,
  ): readonly ShaderAttribute[] {
    const shader = this.requireMaterial(material);
    this.gl.useProgram(shader.program);
    let textureUnit = 0;
    for (const binding of shader.artifact.uniforms) {
      const location = shader.uniforms.get(binding.name);
      if (location === undefined) {
        continue;
      }
      if (binding.source === "automatic") {
        this.uploadAutomaticForDraw(binding, location, automatic);
        this.assertUploadSucceeded(shader, binding);
        continue;
      }
      const value = userValues.get(binding.name);
      if (value === undefined) {
        if (this.debug) {
          throw runtimeError(
            `user uniform \`${binding.name}\` is unset for shader \`${shader.artifact.name}\``,
            shader.artifact.fragmentLocation,
          );
        }
        continue;
      }
      textureUnit = this.uploadUser(binding, location, value, textureUnit);
      this.assertUploadSucceeded(shader, binding);
    }
    return shader.artifact.attributes;
  }

  public updateAutomaticUniforms(
    elapsedSeconds: number,
    width: number,
    height: number,
  ): void {
    for (const shader of this.shaders.values()) {
      this.gl.useProgram(shader.program);
      let textureUnit = 0;
      for (const binding of shader.artifact.uniforms) {
        const location = shader.uniforms.get(binding.name);
        if (location === undefined) {
          continue;
        }
        if (binding.source === "automatic") {
          this.uploadAutomatic(binding, location, elapsedSeconds, width, height);
          this.assertUploadSucceeded(shader, binding);
          continue;
        }
        const value = shader.userValues.get(binding.name);
        if (value === undefined) {
          if (this.debug && shader.globalUniformsSet) {
            throw runtimeError(
              `user uniform \`${binding.name}\` is unset for shader \`${shader.artifact.name}\``,
              shader.artifact.fragmentLocation,
            );
          }
          continue;
        }
        textureUnit = this.uploadUser(binding, location, value, textureUnit);
        this.assertUploadSucceeded(shader, binding);
      }
    }
  }

  public dispose(): void {
    for (const shader of this.shaders.values()) {
      this.gl.deleteProgram(shader.program);
    }
    this.shaders.clear();
  }

  private link(artifact: ShaderArtifact): LinkedShader {
    const vertex = compileArtifactShader(
      this.gl,
      this.gl.VERTEX_SHADER,
      artifact.vertex,
      artifact.name,
      "vertex",
      artifact.vertexLocation,
    );
    let fragment: WebGLShader | undefined;
    let program: WebGLProgram | undefined;
    try {
      fragment = compileArtifactShader(
        this.gl,
        this.gl.FRAGMENT_SHADER,
        artifact.fragment,
        artifact.name,
        "fragment",
        artifact.fragmentLocation,
      );
      program = this.gl.createProgram() ?? undefined;
      if (program === undefined) {
        throw runtimeError(
          `failed to create WebGL2 program for shader \`${artifact.name}\``,
          artifact.vertexLocation,
        );
      }
      this.gl.attachShader(program, vertex);
      this.gl.attachShader(program, fragment);
      this.gl.linkProgram(program);
      if (!this.gl.getProgramParameter(program, this.gl.LINK_STATUS)) {
        const log = this.gl.getProgramInfoLog(program) ?? "unknown link failure";
        throw runtimeError(
          `failed to link shader \`${artifact.name}\`: ${log}`,
          artifact.vertexLocation,
        );
      }

      const uniforms = new Map<string, WebGLUniformLocation>();
      for (const uniform of artifact.uniforms) {
        const location = this.gl.getUniformLocation(program, uniform.glslName);
        if (location !== null) {
          uniforms.set(uniform.name, location);
        }
      }
      return {
        artifact,
        material: createShaderMaterial(artifact.name),
        program,
        uniforms,
        userValues: new Map(),
        globalUniformsSet: false,
      };
    } catch (error) {
      if (program !== undefined) {
        this.gl.deleteProgram(program);
      }
      throw error;
    } finally {
      this.gl.deleteShader(vertex);
      if (fragment !== undefined) {
        this.gl.deleteShader(fragment);
      }
    }
  }

  private uploadAutomatic(
    binding: ShaderUniform,
    location: WebGLUniformLocation,
    elapsedSeconds: number,
    width: number,
    height: number,
  ): void {
    switch (binding.name) {
      case "u_time":
        this.gl.uniform1f(location, elapsedSeconds);
        return;
      case "u_resolution":
        this.gl.uniform2f(location, width, height);
        return;
      case "u_model":
      case "u_view":
      case "u_proj":
        this.gl.uniformMatrix4fv(location, false, IDENTITY_MATRIX);
        return;
      default:
        throw new Error(`unknown automatic uniform \`${binding.name}\``);
    }
  }

  private uploadAutomaticForDraw(
    binding: ShaderUniform,
    location: WebGLUniformLocation,
    automatic: ShaderAutomaticUniforms,
  ): void {
    switch (binding.name) {
      case "u_time":
        this.gl.uniform1f(location, automatic.elapsedSeconds);
        return;
      case "u_resolution":
        this.gl.uniform2f(location, automatic.width, automatic.height);
        return;
      case "u_model":
        this.gl.uniformMatrix4fv(location, false, automatic.model);
        return;
      case "u_view":
        this.gl.uniformMatrix4fv(location, false, automatic.view);
        return;
      case "u_proj":
        this.gl.uniformMatrix4fv(location, false, automatic.projection);
        return;
      default:
        throw new Error(`unknown automatic uniform \`${binding.name}\``);
    }
  }

  private uploadUser(
    binding: ShaderUniform,
    location: WebGLUniformLocation,
    value: ShaderUniformValue,
    textureUnit: number,
  ): number {
    switch (binding.type) {
      case "int":
        this.gl.uniform1i(location, value as number);
        return textureUnit;
      case "float":
        this.gl.uniform1f(location, value as number);
        return textureUnit;
      case "bool":
        this.gl.uniform1i(location, value === true ? 1 : 0);
        return textureUnit;
      case "vec2":
        this.gl.uniform2fv(location, value as readonly number[]);
        return textureUnit;
      case "vec3":
        this.gl.uniform3fv(location, value as readonly number[]);
        return textureUnit;
      case "vec4":
        this.gl.uniform4fv(location, value as readonly number[]);
        return textureUnit;
      case "mat2":
        this.gl.uniformMatrix2fv(location, false, value as readonly number[]);
        return textureUnit;
      case "mat3":
        this.gl.uniformMatrix3fv(location, false, value as readonly number[]);
        return textureUnit;
      case "mat4":
        this.gl.uniformMatrix4fv(location, false, value as readonly number[]);
        return textureUnit;
      case "texture":
        this.gl.activeTexture(this.gl.TEXTURE0 + textureUnit);
        this.gl.bindTexture(this.gl.TEXTURE_2D, value as WebGLTexture);
        this.gl.uniform1i(location, textureUnit);
        return textureUnit + 1;
    }
  }

  private assertUploadSucceeded(
    shader: LinkedShader,
    binding: ShaderUniform,
  ): void {
    const error = this.gl.getError();
    if (error !== this.gl.NO_ERROR) {
      throw runtimeError(
        `WebGL rejected uniform \`${binding.name}\` for shader \`${shader.artifact.name}\` (error 0x${error.toString(16)})`,
        shader.artifact.fragmentLocation,
      );
    }
  }

  private requireMaterial(material: ShaderMaterial): LinkedShader {
    const shader = this.shaders.get(material.shaderName);
    if (shader === undefined || shader.material !== material) {
      throw new Error("shader material belongs to another runtime session");
    }
    return shader;
  }
}

function createShaderMaterial(shaderName: string): ShaderMaterial {
  const material = {
    kind: "shader" as const,
    shaderName,
  };
  Object.defineProperty(material, shaderMaterialBrand, {
    value: true,
  });
  return Object.freeze(material) as ShaderMaterial;
}

function compileArtifactShader(
  gl: WebGL2RenderingContext,
  kind: number,
  source: string,
  name: string,
  stage: "vertex" | "fragment",
  location: SourceLocation,
): WebGLShader {
  const shader = gl.createShader(kind);
  if (shader === null) {
    throw runtimeError(
      `failed to create ${stage} shader for pair \`${name}\``,
      location,
    );
  }
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    const log = gl.getShaderInfoLog(shader) ?? "unknown compilation failure";
    gl.deleteShader(shader);
    throw runtimeError(
      `failed to compile ${stage} shader \`${name}\`: ${log}`,
      location,
    );
  }
  return shader;
}

function validateUniformValue(
  gl: WebGL2RenderingContext,
  binding: ShaderUniform,
  value: ShaderUniformValue,
  location: SourceLocation,
): void {
  const invalid = (): never => {
    throw runtimeError(
      `uniform \`${binding.name}\` expects ${binding.type}`,
      location,
    );
  };
  switch (binding.type) {
    case "int":
      if (
        typeof value !== "number" ||
        !Number.isInteger(value) ||
        value < -2_147_483_648 ||
        value > 2_147_483_647
      ) {
        invalid();
      }
      return;
    case "float":
      if (typeof value !== "number" || !Number.isFinite(value)) invalid();
      return;
    case "bool":
      if (typeof value !== "boolean") invalid();
      return;
    case "vec2":
      validateNumericArray(value, 2, invalid);
      return;
    case "vec3":
      validateNumericArray(value, 3, invalid);
      return;
    case "vec4":
    case "mat2":
      validateNumericArray(value, 4, invalid);
      return;
    case "mat3":
      validateNumericArray(value, 9, invalid);
      return;
    case "mat4":
      validateNumericArray(value, 16, invalid);
      return;
    case "texture":
      if (typeof value !== "object" || value === null || !gl.isTexture(value)) {
        invalid();
      }
  }
}

function validateNumericArray(
  value: ShaderUniformValue,
  length: number,
  invalid: () => never,
): void {
  if (
    !isNumericSequence(value) ||
    value.length !== length ||
    value.some((item) => typeof item !== "number" || !Number.isFinite(item))
  ) {
    invalid();
  }
}

function copyUniformValue(value: ShaderUniformValue): ShaderUniformValue {
  return isNumericSequence(value) ? Object.freeze(Array.from(value)) : value;
}

function isNumericSequence(
  value: ShaderUniformValue,
): value is NumericSequence {
  return Array.isArray(value) || value instanceof Float32Array;
}
