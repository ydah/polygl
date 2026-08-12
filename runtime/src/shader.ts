import { runtimeError } from "./errors.js";
import type { SourceLocation } from "./errors.js";
import { shaderAbi } from "./generated/abi.js";
import {
  validateShaderArtifacts,
  validateShaderBundle,
} from "./validation.js";

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
  readonly shaderAbi?: number;
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
  private readonly materials = new WeakMap<object, LinkedShader>();

  public constructor(
    private readonly gl: WebGL2RenderingContext,
    private readonly debug: boolean,
    artifacts: readonly ShaderArtifact[],
    maxPrograms?: number,
  ) {
    if (typeof debug !== "boolean") {
      throw new TypeError("shader debug flag must be a boolean");
    }
    const validatedArtifacts = validateShaderArtifacts(artifacts);
    if (maxPrograms !== undefined && validatedArtifacts.length > maxPrograms) {
      throw new RangeError(
        `shader program budget exceeded: ${validatedArtifacts.length} > ${maxPrograms}`,
      );
    }
    try {
      for (const artifact of validatedArtifacts) {
        const shader = this.link(artifact);
        this.shaders.set(artifact.name, shader);
        this.materials.set(shader.material, shader);
      }
    } catch (error) {
      this.dispose();
      throw error;
    }
  }

  public static fromBundle(
    gl: WebGL2RenderingContext,
    bundle?: unknown,
    requireShaderAbi = false,
    maxPrograms?: number,
  ): WebGL2ShaderRegistry {
    const validatedBundle = bundle === undefined
      ? undefined
      : validateShaderBundle(bundle);
    if (
      validatedBundle !== undefined &&
      (requireShaderAbi || validatedBundle.shaderAbi !== undefined) &&
      validatedBundle.shaderAbi !== shaderAbi
    ) {
      throw new Error(
        `generated shader bundle requires shader ABI ${String(validatedBundle.shaderAbi ?? "missing")}; this runtime provides shader ABI ${shaderAbi}`,
      );
    }
    return new WebGL2ShaderRegistry(
      gl,
      validatedBundle?.debug ?? false,
      validatedBundle?.shaders ?? [],
      maxPrograms,
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
    return this.materials.has(material);
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

      validateProgramReflection(this.gl, program, artifact);

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
    const shader = this.materials.get(material);
    if (shader === undefined) {
      throw new Error("shader material belongs to another runtime session");
    }
    return shader;
  }
}

function validateProgramReflection(
  gl: WebGL2RenderingContext,
  program: WebGLProgram,
  artifact: ShaderArtifact,
): void {
  for (const binding of artifact.attributes) {
    const actualLocation = gl.getAttribLocation(program, binding.glslName);
    if (actualLocation >= 0 && actualLocation !== binding.location) {
      throw runtimeError(
        `shader \`${artifact.name}\` attribute \`${binding.name}\` declares location ${binding.location}, but the linked program reports ${actualLocation}`,
        artifact.vertexLocation,
      );
    }
  }

  const activeAttributes = reflectedCount(
    gl.getProgramParameter(program, gl.ACTIVE_ATTRIBUTES),
    "attribute",
    artifact,
  );
  const seenAttributes = new Set<string>();
  for (let index = 0; index < activeAttributes; index += 1) {
    const reflected = gl.getActiveAttrib(program, index);
    if (reflected === null) {
      throw runtimeError(
        `shader \`${artifact.name}\` returned no reflection for active attribute ${index}`,
        artifact.vertexLocation,
      );
    }
    const binding = artifact.attributes.find(
      (candidate) => candidate.glslName === reflected.name,
    );
    if (binding === undefined) {
      throw runtimeError(
        `shader \`${artifact.name}\` has unrecorded active attribute \`${reflected.name}\``,
        artifact.vertexLocation,
      );
    }
    if (seenAttributes.has(reflected.name) || reflected.size !== 1) {
      throw runtimeError(
        `shader \`${artifact.name}\` has invalid reflection for attribute \`${reflected.name}\``,
        artifact.vertexLocation,
      );
    }
    seenAttributes.add(reflected.name);
    if (reflected.type !== webGlType(gl, binding.type)) {
      throw runtimeError(
        `shader \`${artifact.name}\` attribute \`${binding.name}\` type does not match the linked program`,
        artifact.vertexLocation,
      );
    }
  }

  const activeUniforms = reflectedCount(
    gl.getProgramParameter(program, gl.ACTIVE_UNIFORMS),
    "uniform",
    artifact,
  );
  const seenUniforms = new Set<string>();
  let activeSamplers = 0;
  for (let index = 0; index < activeUniforms; index += 1) {
    const reflected = gl.getActiveUniform(program, index);
    if (reflected === null) {
      throw runtimeError(
        `shader \`${artifact.name}\` returned no reflection for active uniform ${index}`,
        artifact.fragmentLocation,
      );
    }
    const binding = artifact.uniforms.find(
      (candidate) => candidate.glslName === reflected.name,
    );
    if (binding === undefined) {
      throw runtimeError(
        `shader \`${artifact.name}\` has unrecorded active uniform \`${reflected.name}\``,
        artifact.fragmentLocation,
      );
    }
    if (seenUniforms.has(reflected.name) || reflected.size !== 1) {
      throw runtimeError(
        `shader \`${artifact.name}\` has invalid reflection for uniform \`${reflected.name}\``,
        artifact.fragmentLocation,
      );
    }
    seenUniforms.add(reflected.name);
    if (reflected.type !== webGlType(gl, binding.type)) {
      throw runtimeError(
        `shader \`${artifact.name}\` uniform \`${binding.name}\` type does not match the linked program`,
        artifact.fragmentLocation,
      );
    }
    if (binding.type === "texture") activeSamplers += 1;
  }

  if (activeSamplers > 0) {
    const textureLimit: unknown = gl.getParameter(gl.MAX_TEXTURE_IMAGE_UNITS);
    if (
      typeof textureLimit !== "number" ||
      !Number.isInteger(textureLimit) ||
      textureLimit < 0
    ) {
      throw runtimeError(
        `shader \`${artifact.name}\` could not determine the fragment texture-unit limit`,
        artifact.fragmentLocation,
      );
    }
    if (activeSamplers > textureLimit) {
      throw runtimeError(
        `shader \`${artifact.name}\` requires ${activeSamplers} active texture units, but the device supports ${textureLimit}`,
        artifact.fragmentLocation,
      );
    }
  }
}

function reflectedCount(
  value: unknown,
  kind: string,
  artifact: ShaderArtifact,
): number {
  if (typeof value !== "number" || !Number.isInteger(value) || value < 0) {
    throw runtimeError(
      `shader \`${artifact.name}\` returned an invalid active ${kind} count`,
      artifact.vertexLocation,
    );
  }
  return value;
}

function webGlType(
  gl: WebGL2RenderingContext,
  type: ShaderValueType,
): number {
  switch (type) {
    case "int": return gl.INT;
    case "float": return gl.FLOAT;
    case "bool": return gl.BOOL;
    case "vec2": return gl.FLOAT_VEC2;
    case "vec3": return gl.FLOAT_VEC3;
    case "vec4": return gl.FLOAT_VEC4;
    case "mat2": return gl.FLOAT_MAT2;
    case "mat3": return gl.FLOAT_MAT3;
    case "mat4": return gl.FLOAT_MAT4;
    case "texture": return gl.SAMPLER_2D;
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
