export interface RuntimeCapabilities {
  readonly webglVersion: string;
  readonly shadingLanguageVersion: string;
  readonly maxTextureSize: number;
  readonly maxCombinedTextureImageUnits: number;
  readonly maxFragmentTextureImageUnits: number;
  readonly maxVertexTextureImageUnits: number;
  readonly maxVertexAttributes: number;
  readonly maxVertexUniformVectors: number;
  readonly maxFragmentUniformVectors: number;
  readonly supportedExtensions: readonly string[];
}

export function readRuntimeCapabilities(
  gl: WebGL2RenderingContext,
): RuntimeCapabilities {
  return Object.freeze({
    webglVersion: stringParameter(gl, gl.VERSION, "WebGL version"),
    shadingLanguageVersion: stringParameter(
      gl,
      gl.SHADING_LANGUAGE_VERSION,
      "shading language version",
    ),
    maxTextureSize: positiveIntegerParameter(
      gl,
      gl.MAX_TEXTURE_SIZE,
      "maximum texture size",
    ),
    maxCombinedTextureImageUnits: positiveIntegerParameter(
      gl,
      gl.MAX_COMBINED_TEXTURE_IMAGE_UNITS,
      "combined texture-unit limit",
    ),
    maxFragmentTextureImageUnits: positiveIntegerParameter(
      gl,
      gl.MAX_TEXTURE_IMAGE_UNITS,
      "fragment texture-unit limit",
    ),
    maxVertexTextureImageUnits: nonNegativeIntegerParameter(
      gl,
      gl.MAX_VERTEX_TEXTURE_IMAGE_UNITS,
      "vertex texture-unit limit",
    ),
    maxVertexAttributes: positiveIntegerParameter(
      gl,
      gl.MAX_VERTEX_ATTRIBS,
      "vertex attribute limit",
    ),
    maxVertexUniformVectors: positiveIntegerParameter(
      gl,
      gl.MAX_VERTEX_UNIFORM_VECTORS,
      "vertex uniform-vector limit",
    ),
    maxFragmentUniformVectors: positiveIntegerParameter(
      gl,
      gl.MAX_FRAGMENT_UNIFORM_VECTORS,
      "fragment uniform-vector limit",
    ),
    supportedExtensions: supportedExtensions(gl),
  });
}

function stringParameter(
  gl: WebGL2RenderingContext,
  parameter: number,
  label: string,
): string {
  const value: unknown = gl.getParameter(parameter);
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`WebGL returned an invalid ${label}`);
  }
  return value;
}

function positiveIntegerParameter(
  gl: WebGL2RenderingContext,
  parameter: number,
  label: string,
): number {
  const value = nonNegativeIntegerParameter(gl, parameter, label);
  if (value === 0) throw new Error(`WebGL returned an invalid ${label}`);
  return value;
}

function nonNegativeIntegerParameter(
  gl: WebGL2RenderingContext,
  parameter: number,
  label: string,
): number {
  const value: unknown = gl.getParameter(parameter);
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new Error(`WebGL returned an invalid ${label}`);
  }
  return value;
}

function supportedExtensions(gl: WebGL2RenderingContext): readonly string[] {
  const extensions: unknown = gl.getSupportedExtensions();
  if (extensions === null) return Object.freeze([]);
  if (
    !Array.isArray(extensions) ||
    extensions.some((value) => typeof value !== "string")
  ) {
    throw new Error("WebGL returned an invalid supported-extension list");
  }
  return Object.freeze([...extensions].sort());
}
