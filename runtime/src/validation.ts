import type {
  PolyglProgram,
  PolyglProgramSource,
  RuntimeOptions,
  RuntimeResizeObserver,
} from "./session.js";
import type {
  ShaderArtifact,
  ShaderAttribute,
  ShaderBundle,
  ShaderUniform,
  ShaderValueType,
} from "./shader.js";
import type { RuntimeResourceLimits } from "./scene.js";
import { runtimeError } from "./errors.js";
import type { SourceLocation } from "./errors.js";

const MAX_SHADER_ENTRIES = 4_096;
const GLSL_IDENTIFIER = /^[A-Za-z_][A-Za-z0-9_]*$/;
const VALIDATED_STANDARD_ATTRIBUTES: Readonly<
  Record<string, readonly [string, number, ShaderValueType]>
> = Object.freeze({
  position: ["a_position", 0, "vec3"],
  normal: ["a_normal", 1, "vec3"],
  uv: ["a_uv", 2, "vec2"],
  color: ["a_color", 3, "vec4"],
});
const VALIDATED_AUTOMATIC_UNIFORM_TYPES: Readonly<
  Record<string, ShaderValueType>
> =
  Object.freeze({
    u_time: "float",
    u_resolution: "vec2",
    u_model: "mat4",
    u_view: "mat4",
    u_proj: "mat4",
  });

type DataProperties = ReadonlyMap<string, unknown>;
type AllowedSymbol = (
  key: symbol,
  descriptor: PropertyDescriptor,
  prototype: object | null,
  object: object,
) => boolean;

export function validateRuntimeOptions(value: unknown): RuntimeOptions {
  const properties = dataProperties(value, "runtime options");
  const canvas = optionalObject<HTMLCanvasElement>(properties, "canvas");
  const context = optionalObject<WebGL2RenderingContext>(
    properties,
    "context",
  );
  const documentObject = optionalObject<Document>(properties, "document");
  const requestAnimationFrame = optionalFunction<
    (callback: FrameRequestCallback) => number
  >(properties, "requestAnimationFrame");
  const cancelAnimationFrame = optionalFunction<(handle: number) => void>(
    properties,
    "cancelAnimationFrame",
  );
  const seed = optionalFiniteNumber(properties, "seed");
  const shaderBundleValue = properties.get("shaderBundle");
  const shaderBundle = shaderBundleValue === undefined
    ? undefined
    : validateShaderBundle(shaderBundleValue);
  const imageLoader = optionalFunction<RuntimeOptions["imageLoader"]>(
    properties,
    "imageLoader",
  );
  const resourceLimitsValue = properties.get("resourceLimits");
  const resourceLimits = resourceLimitsValue === undefined
    ? undefined
    : validateResourceLimits(resourceLimitsValue);
  const textureFailurePolicy = optionalStringChoice(
    properties,
    "textureFailurePolicy",
    ["stop", "placeholder"] as const,
  );
  const onTextureError = optionalFunction<RuntimeOptions["onTextureError"]>(
    properties,
    "onTextureError",
  );
  const onError = optionalFunction<(reason: unknown) => void>(
    properties,
    "onError",
  );
  const requireRuntimeAbi = optionalBoolean(properties, "requireRuntimeAbi");
  const maxDeltaSeconds = optionalPositiveFiniteNumber(
    properties,
    "maxDeltaSeconds",
  );
  const fixedDeltaSeconds = optionalPositiveFiniteNumber(
    properties,
    "fixedDeltaSeconds",
  );
  const maxCatchUpSteps = optionalPositiveSafeInteger(
    properties,
    "maxCatchUpSteps",
  );
  const visibilityPolicy = optionalStringChoice(
    properties,
    "visibilityPolicy",
    ["continue", "pause"] as const,
  );
  const autoResize = optionalBoolean(properties, "autoResize");
  const devicePixelRatio = optionalPositiveFiniteNumber(
    properties,
    "devicePixelRatio",
  );
  const createResizeObserver = optionalFunction<
    RuntimeOptions["createResizeObserver"]
  >(properties, "createResizeObserver");
  const focusOnPointerDown = optionalBoolean(properties, "focusOnPointerDown");
  const preventDefaultInput = optionalBoolean(
    properties,
    "preventDefaultInput",
  );

  return Object.freeze({
    ...(canvas === undefined ? {} : { canvas }),
    ...(context === undefined ? {} : { context }),
    ...(documentObject === undefined ? {} : { document: documentObject }),
    ...(requestAnimationFrame === undefined ? {} : { requestAnimationFrame }),
    ...(cancelAnimationFrame === undefined ? {} : { cancelAnimationFrame }),
    ...(seed === undefined ? {} : { seed }),
    ...(shaderBundle === undefined ? {} : { shaderBundle }),
    ...(imageLoader === undefined ? {} : { imageLoader }),
    ...(resourceLimits === undefined ? {} : { resourceLimits }),
    ...(textureFailurePolicy === undefined ? {} : { textureFailurePolicy }),
    ...(onTextureError === undefined ? {} : { onTextureError }),
    ...(onError === undefined ? {} : { onError }),
    ...(requireRuntimeAbi === undefined ? {} : { requireRuntimeAbi }),
    ...(maxDeltaSeconds === undefined ? {} : { maxDeltaSeconds }),
    ...(fixedDeltaSeconds === undefined ? {} : { fixedDeltaSeconds }),
    ...(maxCatchUpSteps === undefined ? {} : { maxCatchUpSteps }),
    ...(visibilityPolicy === undefined ? {} : { visibilityPolicy }),
    ...(autoResize === undefined ? {} : { autoResize }),
    ...(devicePixelRatio === undefined ? {} : { devicePixelRatio }),
    ...(createResizeObserver === undefined ? {} : { createResizeObserver }),
    ...(focusOnPointerDown === undefined ? {} : { focusOnPointerDown }),
    ...(preventDefaultInput === undefined ? {} : { preventDefaultInput }),
  } satisfies RuntimeOptions);
}

function validateResourceLimits(value: unknown): RuntimeResourceLimits {
  const properties = dataProperties(value, "runtime options.resourceLimits");
  const known = [
    "maxMeshes",
    "maxMeshBytes",
    "maxNodes",
    "maxTextures",
    "maxTextureDimension",
    "maxShaderPrograms",
  ] as const;
  for (const name of properties.keys()) {
    if (!known.includes(name as typeof known[number])) {
      throw new TypeError(`unknown runtime resource limit \`${name}\``);
    }
  }
  const maxMeshes = optionalResourceLimit(properties, "maxMeshes");
  const maxMeshBytes = optionalResourceLimit(properties, "maxMeshBytes");
  const maxNodes = optionalResourceLimit(properties, "maxNodes");
  const maxTextures = optionalResourceLimit(properties, "maxTextures");
  const maxTextureDimension = optionalResourceLimit(
    properties,
    "maxTextureDimension",
  );
  const maxShaderPrograms = optionalResourceLimit(
    properties,
    "maxShaderPrograms",
  );
  return Object.freeze({
    ...(maxMeshes === undefined ? {} : { maxMeshes }),
    ...(maxMeshBytes === undefined
      ? {}
      : { maxMeshBytes }),
    ...(maxNodes === undefined ? {} : { maxNodes }),
    ...(maxTextures === undefined
      ? {}
      : { maxTextures }),
    ...(maxTextureDimension === undefined
      ? {}
      : { maxTextureDimension }),
    ...(maxShaderPrograms === undefined
      ? {}
      : { maxShaderPrograms }),
  } satisfies RuntimeResourceLimits);
}

function optionalResourceLimit(
  properties: DataProperties,
  name: string,
): number | undefined {
  const value = properties.get(name);
  return value === undefined
    ? undefined
    : positiveSafeInteger(
      value,
      `runtime options.resourceLimits.${name}`,
    );
}

export function validateProgramSource(value: unknown): PolyglProgramSource {
  return typeof value === "function"
    ? value as PolyglProgramSource
    : validateProgram(value);
}

export function validateProgram(value: unknown): PolyglProgram {
  const properties = dataProperties(
    value,
    "program",
    isModuleNamespaceTag,
  );
  const setup = optionalFunction<PolyglProgram["setup"]>(properties, "setup");
  const frame = optionalFunction<PolyglProgram["frame"]>(properties, "frame");
  const onEvent = optionalFunction<PolyglProgram["on_event"]>(
    properties,
    "on_event",
  );
  const runtimeAbiValue = properties.get("__polyglRuntimeAbi");
  const runtimeAbi = runtimeAbiValue === undefined
    ? undefined
    : nonNegativeInteger(runtimeAbiValue, "program.__polyglRuntimeAbi");
  const shaderBundleValue = properties.get("__polyglShaderBundle");
  const shaderBundle = shaderBundleValue === undefined
    ? undefined
    : validateShaderBundle(shaderBundleValue);

  return {
    ...(setup === undefined ? {} : { setup }),
    ...(frame === undefined ? {} : { frame }),
    ...(onEvent === undefined ? {} : { on_event: onEvent }),
    ...(runtimeAbi === undefined ? {} : { __polyglRuntimeAbi: runtimeAbi }),
    ...(shaderBundle === undefined ? {} : {
      __polyglShaderBundle: shaderBundle,
    }),
  } satisfies PolyglProgram;
}

export function validateResizeObserver(
  value: unknown,
): RuntimeResizeObserver {
  if (!isObject(value)) {
    invalid("resize observer", "an object");
  }
  const observe = safeMethod(value, "observe", "resize observer");
  const disconnect = safeMethod(value, "disconnect", "resize observer");
  return Object.freeze({
    observe: (target: Element) => observe.call(value, target),
    disconnect: () => disconnect.call(value),
  });
}

export function validateShaderBundle(value: unknown): ShaderBundle {
  const properties = dataProperties(value, "shader bundle");
  const shaderAbiValue = properties.get("shaderAbi");
  const bundleShaderAbi = shaderAbiValue === undefined
    ? undefined
    : nonNegativeInteger(shaderAbiValue, "shader bundle.shaderAbi");
  const debug = requiredBoolean(properties, "debug", "shader bundle.debug");
  const shaders = validateShaderArtifacts(
    required(properties, "shaders", "shader bundle.shaders"),
  );
  return Object.freeze({
    ...(bundleShaderAbi === undefined ? {} : { shaderAbi: bundleShaderAbi }),
    debug,
    shaders,
  } satisfies ShaderBundle);
}

export function validateShaderArtifacts(
  value: unknown,
): readonly ShaderArtifact[] {
  const values = denseArray(value, "shader bundle.shaders");
  const names = new Set<string>();
  return Object.freeze(values.map((entry, index) => {
    const artifact = validateShaderArtifact(
      entry,
      `shader bundle.shaders[${index}]`,
    );
    unique(names, artifact.name, "shader name", artifact.vertexLocation);
    return artifact;
  }));
}

function validateShaderArtifact(value: unknown, path: string): ShaderArtifact {
  const properties = dataProperties(value, path);
  const name = requiredNonEmptyString(properties, "name", `${path}.name`);
  const vertex = requiredString(properties, "vertex", `${path}.vertex`);
  const fragment = requiredString(properties, "fragment", `${path}.fragment`);
  const vertexLocation = validateSourceLocation(
    required(properties, "vertexLocation", `${path}.vertexLocation`),
    `${path}.vertexLocation`,
  );
  const fragmentLocation = validateSourceLocation(
    required(properties, "fragmentLocation", `${path}.fragmentLocation`),
    `${path}.fragmentLocation`,
  );
  const attributes = validateAttributes(
    required(properties, "attributes", `${path}.attributes`),
    path,
    vertexLocation,
  );
  const uniforms = validateUniforms(
    required(properties, "uniforms", `${path}.uniforms`),
    path,
    fragmentLocation,
  );
  return Object.freeze({
    name,
    vertex,
    fragment,
    attributes,
    uniforms,
    vertexLocation,
    fragmentLocation,
  });
}

function validateAttributes(
  value: unknown,
  artifactPath: string,
  location: SourceLocation,
): readonly ShaderAttribute[] {
  const values = denseArray(value, `${artifactPath}.attributes`);
  const names = new Set<string>();
  const glslNames = new Set<string>();
  const locations = new Set<number>();
  return Object.freeze(values.map((entry, index) => {
    const path = `${artifactPath}.attributes[${index}]`;
    const properties = dataProperties(entry, path);
    const name = requiredNonEmptyString(properties, "name", `${path}.name`);
    const glslName = requiredGlslName(properties, "glslName", path);
    const attributeLocation = nonNegativeInteger(
      required(properties, "location", `${path}.location`),
      `${path}.location`,
    );
    const type = shaderValueType(
      required(properties, "type", `${path}.type`),
      `${path}.type`,
    );
    validateStandardAttribute(name, glslName, attributeLocation, type, location);
    unique(names, name, "attribute name", location);
    unique(glslNames, glslName, "attribute GLSL name", location);
    unique(locations, attributeLocation, "attribute location", location);
    return Object.freeze({ name, glslName, location: attributeLocation, type });
  }));
}

function validateUniforms(
  value: unknown,
  artifactPath: string,
  location: SourceLocation,
): readonly ShaderUniform[] {
  const values = denseArray(value, `${artifactPath}.uniforms`);
  const names = new Set<string>();
  const glslNames = new Set<string>();
  return Object.freeze(values.map((entry, index) => {
    const path = `${artifactPath}.uniforms[${index}]`;
    const properties = dataProperties(entry, path);
    const name = requiredNonEmptyString(properties, "name", `${path}.name`);
    const glslName = requiredGlslName(properties, "glslName", path);
    const type = shaderValueType(
      required(properties, "type", `${path}.type`),
      `${path}.type`,
    );
    const sourceValue = required(properties, "source", `${path}.source`);
    if (sourceValue !== "automatic" && sourceValue !== "user") {
      invalid(`${path}.source`, '"automatic" or "user"');
    }
    validateAutomaticUniform(name, glslName, type, sourceValue, location);
    unique(names, name, "uniform name", location);
    unique(glslNames, glslName, "uniform GLSL name", location);
    return Object.freeze({ name, glslName, type, source: sourceValue });
  }));
}

function validateSourceLocation(value: unknown, path: string): SourceLocation {
  const properties = dataProperties(value, path);
  const source = requiredNonEmptyString(properties, "source", `${path}.source`);
  const line = positiveSafeInteger(
    required(properties, "line", `${path}.line`),
    `${path}.line`,
  );
  const column = positiveSafeInteger(
    required(properties, "column", `${path}.column`),
    `${path}.column`,
  );
  const start = nonNegativeInteger(
    required(properties, "start", `${path}.start`),
    `${path}.start`,
  );
  const end = nonNegativeInteger(
    required(properties, "end", `${path}.end`),
    `${path}.end`,
  );
  if (end < start) {
    throw new RangeError(`${path}.end must not precede start`);
  }
  return Object.freeze({ source, line, column, start, end });
}

function validateStandardAttribute(
  name: string,
  glslName: string,
  location: number,
  type: ShaderValueType,
  sourceLocation: SourceLocation,
): void {
  const contract = VALIDATED_STANDARD_ATTRIBUTES[name];
  if (
    contract === undefined ||
    glslName !== contract[0] ||
    location !== contract[1] ||
    type !== contract[2]
  ) {
    throw runtimeError(
      `invalid standard mesh attribute metadata for \`${name}\``,
      sourceLocation,
    );
  }
}

function validateAutomaticUniform(
  name: string,
  glslName: string,
  type: ShaderValueType,
  source: "automatic" | "user",
  location: SourceLocation,
): void {
  const expectedType = VALIDATED_AUTOMATIC_UNIFORM_TYPES[name];
  if (source === "automatic") {
    if (expectedType === undefined || glslName !== name || type !== expectedType) {
      throw runtimeError(
        `invalid automatic uniform metadata for \`${name}\``,
        location,
      );
    }
  } else if (expectedType !== undefined) {
    throw runtimeError(
      `reserved automatic uniform \`${name}\` cannot be user-provided`,
      location,
    );
  }
}

function dataProperties(
  value: unknown,
  path: string,
  allowedSymbol?: AllowedSymbol,
): DataProperties {
  if (!isObject(value)) {
    invalid(path, "a plain object");
  }
  const prototype: unknown = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new TypeError(`${path} must not use a custom prototype`);
  }
  const properties = new Map<string, unknown>();
  for (const key of Reflect.ownKeys(value)) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (typeof key !== "string") {
      if (
        descriptor === undefined ||
        allowedSymbol?.(key, descriptor, prototype, value) !== true
      ) {
        throw new TypeError(`${path} must not contain symbol properties`);
      }
      continue;
    }
    if (descriptor === undefined || !("value" in descriptor)) {
      throw new TypeError(`${path}.${key} must be a data property`);
    }
    properties.set(key, descriptor.value);
  }
  return properties;
}

function isModuleNamespaceTag(
  key: symbol,
  descriptor: PropertyDescriptor,
  prototype: object | null,
  object: object,
): boolean {
  return prototype === null &&
    !Object.isExtensible(object) &&
    key === Symbol.toStringTag &&
    "value" in descriptor &&
    descriptor.value === "Module" &&
    descriptor.writable === false &&
    descriptor.enumerable === false &&
    descriptor.configurable === false;
}

function denseArray(value: unknown, path: string): readonly unknown[] {
  if (!Array.isArray(value) || Object.getPrototypeOf(value) !== Array.prototype) {
    invalid(path, "an array");
  }
  if (value.length > MAX_SHADER_ENTRIES) {
    throw new RangeError(
      `${path} cannot contain more than ${MAX_SHADER_ENTRIES} entries`,
    );
  }
  const values: unknown[] = [];
  for (let index = 0; index < value.length; index += 1) {
    const descriptor = Object.getOwnPropertyDescriptor(value, String(index));
    if (descriptor === undefined || !("value" in descriptor)) {
      throw new TypeError(`${path}[${index}] must be a data property`);
    }
    values.push(descriptor.value);
  }
  return values;
}

function safeMethod(
  value: object,
  name: string,
  path: string,
): (...parameters: unknown[]) => unknown {
  let current: object | null = value;
  while (current !== null) {
    const descriptor = Object.getOwnPropertyDescriptor(current, name);
    if (descriptor !== undefined) {
      if (!("value" in descriptor) || typeof descriptor.value !== "function") {
        invalid(`${path}.${name}`, "a data-property function");
      }
      return descriptor.value;
    }
    current = Object.getPrototypeOf(current) as object | null;
  }
  invalid(`${path}.${name}`, "a function");
}

function required(
  properties: DataProperties,
  name: string,
  path: string,
): unknown {
  const value = properties.get(name);
  if (value === undefined) {
    throw new TypeError(`${path} is required`);
  }
  return value;
}

function requiredString(
  properties: DataProperties,
  name: string,
  path: string,
): string {
  const value = required(properties, name, path);
  if (typeof value !== "string") invalid(path, "a string");
  return value;
}

function requiredNonEmptyString(
  properties: DataProperties,
  name: string,
  path: string,
): string {
  const value = requiredString(properties, name, path);
  if (value.length === 0) invalid(path, "a non-empty string");
  return value;
}

function requiredGlslName(
  properties: DataProperties,
  name: string,
  path: string,
): string {
  const value = requiredNonEmptyString(properties, name, `${path}.${name}`);
  if (!GLSL_IDENTIFIER.test(value)) {
    invalid(`${path}.${name}`, "a GLSL identifier");
  }
  return value;
}

function requiredBoolean(
  properties: DataProperties,
  name: string,
  path: string,
): boolean {
  const value = required(properties, name, path);
  if (typeof value !== "boolean") invalid(path, "a boolean");
  return value;
}

function optionalFunction<T>(
  properties: DataProperties,
  name: string,
): T | undefined {
  const value = properties.get(name);
  if (value === undefined) return undefined;
  if (typeof value !== "function") invalid(`runtime boundary.${name}`, "a function");
  return value as T;
}

function optionalObject<T>(
  properties: DataProperties,
  name: string,
): T | undefined {
  const value = properties.get(name);
  if (value === undefined) return undefined;
  if (!isObject(value)) invalid(`runtime options.${name}`, "an object");
  return value as T;
}

function optionalBoolean(
  properties: DataProperties,
  name: string,
): boolean | undefined {
  const value = properties.get(name);
  if (value === undefined) return undefined;
  if (typeof value !== "boolean") invalid(`runtime options.${name}`, "a boolean");
  return value;
}

function optionalFiniteNumber(
  properties: DataProperties,
  name: string,
): number | undefined {
  const value = properties.get(name);
  if (value === undefined) return undefined;
  if (typeof value !== "number" || !Number.isFinite(value)) {
    invalid(`runtime options.${name}`, "a finite number");
  }
  return value;
}

function optionalPositiveFiniteNumber(
  properties: DataProperties,
  name: string,
): number | undefined {
  const value = optionalFiniteNumber(properties, name);
  if (value !== undefined && value <= 0) {
    invalid(`runtime options.${name}`, "a finite number greater than zero");
  }
  return value;
}

function optionalPositiveSafeInteger(
  properties: DataProperties,
  name: string,
): number | undefined {
  const value = properties.get(name);
  if (value === undefined) return undefined;
  const integer = positiveSafeInteger(value, `runtime options.${name}`);
  return integer;
}

function optionalStringChoice<const T extends readonly string[]>(
  properties: DataProperties,
  name: string,
  choices: T,
): T[number] | undefined {
  const value = properties.get(name);
  if (value === undefined) return undefined;
  if (typeof value !== "string" || !choices.includes(value)) {
    invalid(`runtime options.${name}`, choices.map((item) => `"${item}"`).join(" or "));
  }
  return value;
}

function shaderValueType(value: unknown, path: string): ShaderValueType {
  switch (value) {
    case "int":
    case "float":
    case "bool":
    case "vec2":
    case "vec3":
    case "vec4":
    case "mat2":
    case "mat3":
    case "mat4":
    case "texture":
      return value;
    default:
      invalid(path, "a supported shader value type");
  }
}

function nonNegativeInteger(value: unknown, path: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    invalid(path, "a non-negative safe integer");
  }
  return value;
}

function positiveSafeInteger(value: unknown, path: string): number {
  const integer = nonNegativeInteger(value, path);
  if (integer === 0) invalid(path, "a positive safe integer");
  return integer;
}

function unique<T>(
  values: Set<T>,
  value: T,
  description: string,
  location: SourceLocation,
): void {
  if (values.has(value)) {
    throw runtimeError(
      `duplicate ${description} \`${String(value)}\` in shader metadata`,
      location,
    );
  }
  values.add(value);
}

function isObject(value: unknown): value is object {
  return typeof value === "object" && value !== null;
}

function invalid(path: string, expectation: string): never {
  throw new TypeError(`${path} must be ${expectation}`);
}
