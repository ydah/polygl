import {
  lookAt4,
  model4,
  normal3,
  normalize3,
  perspective4,
} from "./math3d.js";
import type { Mat4, Vec3 } from "./math3d.js";
import {
  boxMesh,
  customMesh,
  FLOATS_PER_MESH_VERTEX,
  planeMesh,
  sphereMesh,
} from "./mesh.js";
import type { MeshData } from "./mesh.js";
import type {
  NumericSequence,
  ShaderAttribute,
  ShaderMaterial,
  ShaderUniformValue,
} from "./shader.js";
import { WebGL2ShaderRegistry } from "./shader.js";

const sceneOwnerBrand: unique symbol = Symbol("SceneOwner");

export interface MeshHandle {
  readonly kind: "mesh";
  readonly [sceneOwnerBrand]: object;
}

export interface NodeHandle {
  readonly kind: "node";
  readonly [sceneOwnerBrand]: object;
}

export interface BasicMaterial {
  readonly kind: "basic";
  readonly color: readonly [number, number, number, number];
  readonly [sceneOwnerBrand]: object;
}

export interface TextureHandle {
  readonly kind: "texture";
  readonly path: string;
  readonly loaded: boolean;
  readonly [sceneOwnerBrand]: object;
}

export type MaterialHandle = BasicMaterial | ShaderMaterial;
export type SceneShaderValue =
  | number
  | boolean
  | NumericSequence
  | TextureHandle;
export type RuntimeImageLoader = (url: string) => Promise<TexImageSource>;

interface MeshResource {
  readonly handle: MeshHandle;
  readonly vertexBuffer: WebGLBuffer;
  readonly indexBuffer: WebGLBuffer;
  readonly indexCount: number;
  references: number;
}

interface SceneNode {
  readonly handle: NodeHandle;
  readonly mesh: MeshResource;
  readonly material: MaterialHandle;
  position: Vec3;
  rotation: Vec3;
  scale: Vec3;
  readonly uniforms: Map<string, ShaderUniformValue>;
  readonly textureUniforms: Map<string, SceneTexture>;
}

interface SceneTexture {
  readonly handle: TextureHandle;
  readonly path: string;
  readonly texture: WebGLTexture;
  loaded: boolean;
  disposed: boolean;
  references: number;
}

interface CameraState {
  verticalFov: number;
  near: number;
  far: number;
  eye: Vec3;
  target: Vec3;
  up: Vec3;
}

interface LightState {
  direction: Vec3;
  color: Vec3;
}

interface BasicProgram {
  readonly program: WebGLProgram;
  readonly model: WebGLUniformLocation;
  readonly view: WebGLUniformLocation;
  readonly projection: WebGLUniformLocation;
  readonly normal: WebGLUniformLocation;
  readonly materialColor: WebGLUniformLocation;
  readonly lightDirection: WebGLUniformLocation;
  readonly lightColor: WebGLUniformLocation;
  readonly eye: WebGLUniformLocation;
}

export class WebGL2SceneRenderer {
  private readonly owner = {};
  private readonly meshes = new Set<MeshResource>();
  private readonly nodes = new Set<SceneNode>();
  private readonly textures = new Map<string, SceneTexture>();
  private readonly meshHandles = new WeakMap<MeshHandle, MeshResource>();
  private readonly nodeHandles = new WeakMap<NodeHandle, SceneNode>();
  private readonly textureHandles = new WeakMap<TextureHandle, SceneTexture>();
  private readonly pendingSetupAssets = new Set<Promise<void>>();
  private shaderRegistry: WebGL2ShaderRegistry;
  private basicProgram: BasicProgram | undefined;
  private startupPhase = true;
  private disposed = false;
  private camera: CameraState = {
    verticalFov: Math.PI / 4,
    near: 0.1,
    far: 100,
    eye: [0, 0, 5],
    target: [0, 0, 0],
    up: [0, 1, 0],
  };
  private light: LightState = {
    direction: normalize3([-0.5, -1, -0.5]),
    color: [1, 1, 1],
  };

  public constructor(
    private readonly gl: WebGL2RenderingContext,
    shaderRegistry: WebGL2ShaderRegistry,
    private readonly documentObject: Document | undefined,
    private readonly imageLoader: RuntimeImageLoader = defaultImageLoader,
    private readonly onAsyncError: (reason: unknown) => void = () => {},
  ) {
    this.shaderRegistry = shaderRegistry;
  }

  public replaceShaderRegistry(registry: WebGL2ShaderRegistry): void {
    this.shaderRegistry = registry;
  }

  public meshBox(width: number, height: number, depth: number): MeshHandle {
    return this.createMesh(boxMesh(width, height, depth));
  }

  public meshSphere(radius: number, segments: number): MeshHandle {
    return this.createMesh(sphereMesh(radius, segments));
  }

  public meshPlane(
    width: number,
    depth: number,
    columns = 1,
    rows = 1,
  ): MeshHandle {
    return this.createMesh(planeMesh(width, depth, columns, rows));
  }

  public meshFrom(
    vertices: readonly number[],
    indices: readonly number[],
  ): MeshHandle {
    return this.createMesh(customMesh(vertices, indices));
  }

  public materialBasic(
    color: NumericSequence,
  ): BasicMaterial {
    const safeColor = fixedVector(color, 4, "basic material color");
    const material = brand({
      kind: "basic" as const,
      color: Object.freeze([...safeColor]) as unknown as readonly [
        number,
        number,
        number,
        number,
      ],
    }, this.owner);
    return Object.freeze(material) as BasicMaterial;
  }

  public nodeAdd(mesh: MeshHandle, material: MaterialHandle): NodeHandle {
    const resource = this.requireMesh(mesh);
    if (material.kind === "basic") {
      this.requireOwned(material, "material");
    } else if (!this.shaderRegistry.owns(material)) {
      throw new Error("shader material belongs to another runtime session");
    }
    const handle = opaqueHandle("node", this.owner) as NodeHandle;
    const node: SceneNode = {
      handle,
      mesh: resource,
      material,
      position: [0, 0, 0],
      rotation: [0, 0, 0],
      scale: [1, 1, 1],
      uniforms: new Map(),
      textureUniforms: new Map(),
    };
    this.nodes.add(node);
    this.nodeHandles.set(handle, node);
    resource.references += 1;
    return handle;
  }

  public nodeRemove(node: NodeHandle): void {
    const resource = this.requireNode(node);
    this.nodes.delete(resource);
    resource.mesh.references -= 1;
    for (const texture of resource.textureUniforms.values()) {
      texture.references -= 1;
    }
    resource.textureUniforms.clear();
    resource.uniforms.clear();
  }

  public meshDispose(mesh: MeshHandle): void {
    const resource = this.requireMesh(mesh);
    if (resource.references > 0) {
      throw new Error(
        `mesh is still referenced by ${resource.references} scene node(s)`,
      );
    }
    this.meshes.delete(resource);
    this.gl.deleteBuffer(resource.vertexBuffer);
    this.gl.deleteBuffer(resource.indexBuffer);
  }

  public nodeSetPosition(node: NodeHandle, x: number, y: number, z: number): void {
    this.requireNode(node).position = finiteVec3([x, y, z], "node position");
  }

  public nodeSetRotation(node: NodeHandle, x: number, y: number, z: number): void {
    this.requireNode(node).rotation = finiteVec3([x, y, z], "node rotation");
  }

  public nodeSetScale(node: NodeHandle, x: number, y: number, z: number): void {
    const scale = finiteVec3([x, y, z], "node scale");
    if (scale.some((value) => Math.abs(value) <= 1e-12)) {
      throw new RangeError("node scale components must not be zero");
    }
    this.requireNode(node).scale = scale;
  }

  public cameraPerspective(verticalFov: number, near: number, far: number): void {
    if (
      !Number.isFinite(verticalFov) ||
      verticalFov <= 0 ||
      verticalFov >= Math.PI
    ) {
      throw new RangeError(
        "camera field of view must be in radians between 0 and pi",
      );
    }
    if (
      !Number.isFinite(near) ||
      !Number.isFinite(far) ||
      near <= 0 ||
      far <= near
    ) {
      throw new RangeError("camera clipping planes must satisfy 0 < near < far");
    }
    this.camera = { ...this.camera, verticalFov, near, far };
  }

  public cameraLookAt(
    eye: NumericSequence,
    target: NumericSequence,
    up: NumericSequence,
  ): void {
    const safeEye = fixedVec3(eye, "camera eye");
    const safeTarget = fixedVec3(target, "camera target");
    const safeUp = fixedVec3(up, "camera up");
    lookAt4(safeEye, safeTarget, safeUp);
    this.camera = {
      ...this.camera,
      eye: safeEye,
      target: safeTarget,
      up: safeUp,
    };
  }

  public lightDirectional(
    direction: NumericSequence,
    color: NumericSequence,
  ): void {
    const safeColor = fixedVec3(color, "directional light color");
    if (safeColor.some((value) => value < 0)) {
      throw new RangeError("directional light color must not be negative");
    }
    this.light = {
      direction: normalize3(
        fixedVec3(direction, "directional light direction"),
        "directional light direction",
      ),
      color: safeColor,
    };
  }

  public textureLoad(path: string): TextureHandle {
    const safePath = validateAssetPath(path);
    const cached = this.textures.get(safePath);
    if (cached !== undefined) {
      return cached.handle;
    }
    const texture = this.gl.createTexture();
    if (texture === null) {
      throw new Error(`failed to create texture for \`${safePath}\``);
    }
    this.gl.bindTexture(this.gl.TEXTURE_2D, texture);
    this.gl.texParameteri(
      this.gl.TEXTURE_2D,
      this.gl.TEXTURE_MIN_FILTER,
      this.gl.LINEAR,
    );
    this.gl.texParameteri(
      this.gl.TEXTURE_2D,
      this.gl.TEXTURE_MAG_FILTER,
      this.gl.LINEAR,
    );
    this.gl.texParameteri(
      this.gl.TEXTURE_2D,
      this.gl.TEXTURE_WRAP_S,
      this.gl.CLAMP_TO_EDGE,
    );
    this.gl.texParameteri(
      this.gl.TEXTURE_2D,
      this.gl.TEXTURE_WRAP_T,
      this.gl.CLAMP_TO_EDGE,
    );
    this.gl.texImage2D(
      this.gl.TEXTURE_2D,
      0,
      this.gl.RGBA,
      1,
      1,
      0,
      this.gl.RGBA,
      this.gl.UNSIGNED_BYTE,
      new Uint8Array([255, 255, 255, 255]),
    );
    let resource: SceneTexture;
    const handle = brand(
      {
        kind: "texture" as const,
        path: safePath,
        get loaded(): boolean {
          return resource.loaded;
        },
      },
      this.owner,
    ) as TextureHandle;
    Object.freeze(handle);
    resource = {
      handle,
      path: safePath,
      texture,
      loaded: false,
      disposed: false,
      references: 0,
    };
    this.textures.set(safePath, resource);
    this.textureHandles.set(handle, resource);
    const load = this.loadTexture(resource);
    if (this.startupPhase) {
      this.pendingSetupAssets.add(load);
    } else {
      void load.catch(this.onAsyncError);
    }
    return handle;
  }

  public textureDispose(texture: TextureHandle): void {
    const resource = this.requireTexture(texture);
    if (resource.references > 0) {
      throw new Error(
        `texture is still referenced by ${resource.references} scene node uniform(s)`,
      );
    }
    resource.disposed = true;
    this.textures.delete(resource.path);
    this.gl.deleteTexture(resource.texture);
  }

  public shaderSet(
    node: NodeHandle,
    uniformName: string,
    value: SceneShaderValue,
  ): void {
    const sceneNode = this.requireNode(node);
    if (sceneNode.material.kind !== "shader") {
      throw new Error("shader_set requires a node with a shader material");
    }
    let uploadValue: ShaderUniformValue;
    let textureResource: SceneTexture | undefined;
    if (isTextureHandle(value)) {
      textureResource = this.requireTexture(value);
      uploadValue = textureResource.texture;
    } else {
      uploadValue = value;
    }
    const normalized = this.shaderRegistry.nodeUniform(
      sceneNode.material,
      uniformName,
      uploadValue,
    );
    const previousTexture = sceneNode.textureUniforms.get(uniformName);
    if (previousTexture !== textureResource) {
      if (previousTexture !== undefined) {
        previousTexture.references -= 1;
        sceneNode.textureUniforms.delete(uniformName);
      }
      if (textureResource !== undefined) {
        textureResource.references += 1;
        sceneNode.textureUniforms.set(uniformName, textureResource);
      }
    }
    sceneNode.uniforms.set(uniformName, normalized);
  }

  public async awaitSetupAssets(): Promise<void> {
    while (this.pendingSetupAssets.size > 0) {
      const pending = [...this.pendingSetupAssets];
      this.pendingSetupAssets.clear();
      await Promise.all(pending);
    }
    this.startupPhase = false;
  }

  public render(
    elapsedSeconds: number,
    width: number,
    height: number,
  ): void {
    if (this.nodes.size === 0) {
      return;
    }
    const view = lookAt4(this.camera.eye, this.camera.target, this.camera.up);
    const projection = perspective4(
      this.camera.verticalFov,
      Math.max(1, width) / Math.max(1, height),
      this.camera.near,
      this.camera.far,
    );
    this.gl.enable(this.gl.DEPTH_TEST);
    this.gl.depthFunc(this.gl.LEQUAL);
    this.gl.clear(this.gl.DEPTH_BUFFER_BIT);
    for (const node of this.nodes) {
      const model = model4(node.position, node.rotation, node.scale);
      if (node.material.kind === "basic") {
        this.bindBasic(node.material, model, view, projection);
        this.bindMesh(node.mesh, STANDARD_ATTRIBUTES);
      } else {
        const attributes = this.shaderRegistry.bindForDraw(
          node.material,
          node.uniforms,
          {
            elapsedSeconds,
            width,
            height,
            model,
            view,
            projection,
          },
        );
        this.bindMesh(node.mesh, attributes);
      }
      this.gl.drawElements(
        this.gl.TRIANGLES,
        node.mesh.indexCount,
        this.gl.UNSIGNED_INT,
        0,
      );
    }
    this.gl.disable(this.gl.DEPTH_TEST);
  }

  public dispose(): void {
    if (this.disposed) {
      return;
    }
    this.disposed = true;
    for (const mesh of this.meshes) {
      this.gl.deleteBuffer(mesh.vertexBuffer);
      this.gl.deleteBuffer(mesh.indexBuffer);
    }
    for (const texture of this.textures.values()) {
      texture.disposed = true;
      this.gl.deleteTexture(texture.texture);
    }
    if (this.basicProgram !== undefined) {
      this.gl.deleteProgram(this.basicProgram.program);
    }
    this.meshes.clear();
    this.nodes.clear();
    this.textures.clear();
    this.pendingSetupAssets.clear();
  }

  private createMesh(data: MeshData): MeshHandle {
    const vertexBuffer = this.gl.createBuffer();
    const indexBuffer = this.gl.createBuffer();
    if (vertexBuffer === null || indexBuffer === null) {
      if (vertexBuffer !== null) this.gl.deleteBuffer(vertexBuffer);
      if (indexBuffer !== null) this.gl.deleteBuffer(indexBuffer);
      throw new Error("failed to create mesh buffers");
    }
    this.gl.bindBuffer(this.gl.ARRAY_BUFFER, vertexBuffer);
    this.gl.bufferData(this.gl.ARRAY_BUFFER, data.vertices, this.gl.STATIC_DRAW);
    this.gl.bindBuffer(this.gl.ELEMENT_ARRAY_BUFFER, indexBuffer);
    this.gl.bufferData(
      this.gl.ELEMENT_ARRAY_BUFFER,
      data.indices,
      this.gl.STATIC_DRAW,
    );
    const handle = opaqueHandle("mesh", this.owner) as MeshHandle;
    const mesh: MeshResource = {
      handle,
      vertexBuffer,
      indexBuffer,
      indexCount: data.indices.length,
      references: 0,
    };
    this.meshes.add(mesh);
    this.meshHandles.set(handle, mesh);
    return handle;
  }

  private bindMesh(
    mesh: MeshResource,
    attributes: readonly ShaderAttribute[],
  ): void {
    this.gl.bindBuffer(this.gl.ARRAY_BUFFER, mesh.vertexBuffer);
    this.gl.bindBuffer(this.gl.ELEMENT_ARRAY_BUFFER, mesh.indexBuffer);
    const stride =
      FLOATS_PER_MESH_VERTEX * Float32Array.BYTES_PER_ELEMENT;
    for (const attribute of attributes) {
      const layout = ATTRIBUTE_LAYOUT[attribute.name];
      if (layout === undefined) {
        throw new Error(`unknown standard mesh attribute \`${attribute.name}\``);
      }
      this.gl.enableVertexAttribArray(attribute.location);
      this.gl.vertexAttribPointer(
        attribute.location,
        layout.size,
        this.gl.FLOAT,
        false,
        stride,
        layout.offset * Float32Array.BYTES_PER_ELEMENT,
      );
    }
  }

  private bindBasic(
    material: BasicMaterial,
    model: Mat4,
    view: Mat4,
    projection: Mat4,
  ): void {
    const basic = this.basicProgram ?? this.createBasicProgram();
    this.basicProgram = basic;
    this.gl.useProgram(basic.program);
    this.gl.uniformMatrix4fv(basic.model, false, model);
    this.gl.uniformMatrix4fv(basic.view, false, view);
    this.gl.uniformMatrix4fv(basic.projection, false, projection);
    this.gl.uniformMatrix3fv(basic.normal, false, normal3(model));
    this.gl.uniform4fv(basic.materialColor, material.color);
    this.gl.uniform3fv(basic.lightDirection, this.light.direction);
    this.gl.uniform3fv(basic.lightColor, this.light.color);
    this.gl.uniform3fv(basic.eye, this.camera.eye);
  }

  private createBasicProgram(): BasicProgram {
    const program = sceneLinkProgram(
      this.gl,
      BASIC_VERTEX_SHADER,
      BASIC_FRAGMENT_SHADER,
    );
    try {
      return {
        program,
        model: requiredUniform(this.gl, program, "u_model"),
        view: requiredUniform(this.gl, program, "u_view"),
        projection: requiredUniform(this.gl, program, "u_proj"),
        normal: requiredUniform(this.gl, program, "u_normal"),
        materialColor: requiredUniform(
          this.gl,
          program,
          "u_material_color",
        ),
        lightDirection: requiredUniform(
          this.gl,
          program,
          "u_light_direction",
        ),
        lightColor: requiredUniform(this.gl, program, "u_light_color"),
        eye: requiredUniform(this.gl, program, "u_eye"),
      };
    } catch (error) {
      this.gl.deleteProgram(program);
      throw error;
    }
  }

  private async loadTexture(handle: SceneTexture): Promise<void> {
    const url = resolveAssetUrl(handle.path, this.documentObject);
    let image: TexImageSource;
    try {
      image = await this.imageLoader(url);
    } catch (error) {
      if (
        this.disposed ||
        handle.disposed ||
        this.textures.get(handle.path) !== handle
      ) {
        return;
      }
      throw new Error(
        `failed to load texture \`${handle.path}\`: ${errorMessage(error)}`,
      );
    }
    if (
      this.disposed ||
      handle.disposed ||
      this.textures.get(handle.path) !== handle
    ) {
      return;
    }
    this.gl.bindTexture(this.gl.TEXTURE_2D, handle.texture);
    this.gl.texImage2D(
      this.gl.TEXTURE_2D,
      0,
      this.gl.RGBA,
      this.gl.RGBA,
      this.gl.UNSIGNED_BYTE,
      image,
    );
    handle.loaded = true;
  }

  private requireMesh(mesh: MeshHandle): MeshResource {
    this.requireOwned(mesh, "mesh");
    const resource = this.meshHandles.get(mesh);
    if (resource === undefined || !this.meshes.has(resource)) {
      throw new Error("mesh handle is no longer valid");
    }
    return resource;
  }

  private requireNode(node: NodeHandle): SceneNode {
    this.requireOwned(node, "node");
    const sceneNode = this.nodeHandles.get(node);
    if (sceneNode === undefined || !this.nodes.has(sceneNode)) {
      throw new Error("node handle is no longer valid");
    }
    return sceneNode;
  }

  private requireTexture(texture: TextureHandle): SceneTexture {
    this.requireOwned(texture, "texture");
    const resource = this.textureHandles.get(texture);
    if (
      resource === undefined ||
      resource.disposed ||
      this.textures.get(resource.path) !== resource
    ) {
      throw new Error("texture handle is no longer valid");
    }
    return resource;
  }

  private requireOwned(
    handle: { readonly [sceneOwnerBrand]: object },
    kind: string,
  ): void {
    if (handle[sceneOwnerBrand] !== this.owner) {
      throw new Error(`${kind} belongs to another runtime session`);
    }
  }
}

const ATTRIBUTE_LAYOUT: Readonly<
  Record<string, { readonly size: number; readonly offset: number }>
> = Object.freeze({
  position: { size: 3, offset: 0 },
  normal: { size: 3, offset: 3 },
  uv: { size: 2, offset: 6 },
  color: { size: 4, offset: 8 },
});

const STANDARD_ATTRIBUTES: readonly ShaderAttribute[] = Object.freeze([
  {
    name: "position",
    glslName: "a_position",
    location: 0,
    type: "vec3",
  },
  {
    name: "normal",
    glslName: "a_normal",
    location: 1,
    type: "vec3",
  },
  {
    name: "uv",
    glslName: "a_uv",
    location: 2,
    type: "vec2",
  },
  {
    name: "color",
    glslName: "a_color",
    location: 3,
    type: "vec4",
  },
]);

const BASIC_VERTEX_SHADER = `#version 300 es
precision highp float;
layout(location = 0) in vec3 a_position;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec2 a_uv;
layout(location = 3) in vec4 a_color;
uniform mat4 u_model;
uniform mat4 u_view;
uniform mat4 u_proj;
uniform mat3 u_normal;
out vec3 v_world_position;
out vec3 v_normal;
out vec4 v_color;

void main() {
  vec4 world = u_model * vec4(a_position, 1.0);
  gl_Position = u_proj * u_view * world;
  v_world_position = world.xyz;
  v_normal = normalize(u_normal * a_normal);
  v_color = a_color;
}`;

const BASIC_FRAGMENT_SHADER = `#version 300 es
precision highp float;
in vec3 v_world_position;
in vec3 v_normal;
in vec4 v_color;
uniform vec4 u_material_color;
uniform vec3 u_light_direction;
uniform vec3 u_light_color;
uniform vec3 u_eye;
out vec4 out_color;

void main() {
  vec3 normal = normalize(v_normal);
  vec3 light = normalize(-u_light_direction);
  vec3 view = normalize(u_eye - v_world_position);
  vec3 half_vector = normalize(light + view);
  float diffuse = max(dot(normal, light), 0.0);
  float specular = pow(max(dot(normal, half_vector), 0.0), 32.0);
  vec3 lighting = vec3(0.12) + u_light_color * (diffuse + specular * 0.35);
  out_color = vec4(
    u_material_color.rgb * v_color.rgb * lighting,
    u_material_color.a * v_color.a
  );
}`;

function sceneLinkProgram(
  gl: WebGL2RenderingContext,
  vertexSource: string,
  fragmentSource: string,
): WebGLProgram {
  const vertex = compileSceneShader(gl, gl.VERTEX_SHADER, vertexSource);
  let fragment: WebGLShader | undefined;
  let program: WebGLProgram | undefined;
  try {
    fragment = compileSceneShader(gl, gl.FRAGMENT_SHADER, fragmentSource);
    program = gl.createProgram() ?? undefined;
    if (program === undefined) {
      throw new Error("failed to create the built-in 3D program");
    }
    gl.attachShader(program, vertex);
    gl.attachShader(program, fragment);
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
      const log = gl.getProgramInfoLog(program) ?? "unknown link failure";
      throw new Error(`failed to link the built-in 3D program: ${log}`);
    }
    return program;
  } catch (error) {
    if (program !== undefined) gl.deleteProgram(program);
    throw error;
  } finally {
    gl.deleteShader(vertex);
    if (fragment !== undefined) gl.deleteShader(fragment);
  }
}

function compileSceneShader(
  gl: WebGL2RenderingContext,
  kind: number,
  source: string,
): WebGLShader {
  const shader = gl.createShader(kind);
  if (shader === null) {
    throw new Error("failed to create a built-in 3D shader");
  }
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    const log = gl.getShaderInfoLog(shader) ?? "unknown compilation failure";
    gl.deleteShader(shader);
    throw new Error(`failed to compile a built-in 3D shader: ${log}`);
  }
  return shader;
}

function requiredUniform(
  gl: WebGL2RenderingContext,
  program: WebGLProgram,
  name: string,
): WebGLUniformLocation {
  const location = gl.getUniformLocation(program, name);
  if (location === null) {
    throw new Error(`the built-in 3D shader is missing uniform \`${name}\``);
  }
  return location;
}

function brand<T extends object>(value: T, owner: object): T {
  Object.defineProperty(value, sceneOwnerBrand, { value: owner });
  return value;
}

function opaqueHandle(kind: "mesh" | "node", owner: object): object {
  return Object.freeze(brand({ kind }, owner));
}

function fixedVector(
  value: NumericSequence,
  length: number,
  label: string,
): readonly number[] {
  const components = Array.from(value);
  if (
    components.length !== length ||
    components.some((item) => !Number.isFinite(item))
  ) {
    throw new RangeError(`${label} must contain ${length} finite numbers`);
  }
  return Object.freeze(components);
}

function fixedVec3(value: NumericSequence, label: string): Vec3 {
  return fixedVector(value, 3, label) as Vec3;
}

function finiteVec3(value: Vec3, label: string): Vec3 {
  if (value.some((component) => !Number.isFinite(component))) {
    throw new RangeError(`${label} must contain finite numbers`);
  }
  return value;
}

function validateAssetPath(path: string): string {
  if (
    path.length === 0 ||
    path.startsWith("/") ||
    path.includes("\\") ||
    path.includes("://") ||
    path.split("/").some((part) => part === "" || part === "." || part === "..")
  ) {
    throw new Error(
      "texture path must be a non-empty relative slash-separated path",
    );
  }
  return path;
}

function resolveAssetUrl(path: string, documentObject: Document | undefined): string {
  const base = documentObject?.baseURI ?? globalThis.location?.href;
  return base === undefined ? path : new URL(path, base).href;
}

async function defaultImageLoader(url: string): Promise<TexImageSource> {
  if (typeof globalThis.Image !== "function") {
    throw new Error("image loading is unavailable outside a browser");
  }
  return await new Promise<HTMLImageElement>((resolve, reject) => {
    const image = new globalThis.Image();
    image.addEventListener("load", () => resolve(image), { once: true });
    image.addEventListener(
      "error",
      () => reject(new Error(`browser could not decode ${url}`)),
      { once: true },
    );
    image.src = url;
  });
}

function isTextureHandle(value: SceneShaderValue): value is TextureHandle {
  return typeof value === "object" && value !== null && "kind" in value &&
    value.kind === "texture";
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
