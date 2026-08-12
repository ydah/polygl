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
import { copyFixedFiniteSequence } from "./numeric.js";
import type {
  NumericSequence,
  ShaderAttribute,
  ShaderMaterial,
  ShaderUniformValue,
} from "./shader.js";
import { WebGLStateCache } from "./webgl-state.js";
import { WebGL2ShaderRegistry } from "./shader.js";

const sceneOwnerBrand: unique symbol = Symbol("SceneOwner");
const sceneHandleInfo = new WeakMap<
  object,
  { readonly owner: object; readonly kind: string }
>();

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

export interface RuntimeImageRequest {
  readonly signal: AbortSignal;
}

export interface RuntimeResourceLimits {
  readonly maxMeshes?: number;
  readonly maxMeshBytes?: number;
  readonly maxNodes?: number;
  readonly maxTextures?: number;
  readonly maxTextureDimension?: number;
  readonly maxShaderPrograms?: number;
}

export interface SceneRendererStats {
  readonly drawCalls: number;
  readonly triangles: number;
  readonly meshes: number;
  readonly meshBytes: number;
  readonly nodes: number;
  readonly textures: number;
  readonly culledNodes: number;
}

export interface TextureOptions {
  readonly minFilter?:
    | "nearest"
    | "linear"
    | "nearest-mipmap-nearest"
    | "linear-mipmap-nearest"
    | "nearest-mipmap-linear"
    | "linear-mipmap-linear";
  readonly magFilter?: "nearest" | "linear";
  readonly wrapS?: "clamp-to-edge" | "repeat" | "mirrored-repeat";
  readonly wrapT?: "clamp-to-edge" | "repeat" | "mirrored-repeat";
  readonly mipmaps?: boolean;
  readonly flipY?: boolean;
  readonly premultiplyAlpha?: boolean;
  readonly colorSpaceConversion?: "browser-default" | "none";
}

export type MaterialHandle = BasicMaterial | ShaderMaterial;
export type SceneShaderValue =
  | number
  | boolean
  | NumericSequence
  | TextureHandle;
export type RuntimeImageLoader = (
  url: string,
  request: RuntimeImageRequest,
) => Promise<TexImageSource>;
export type TextureFailurePolicy = "stop" | "placeholder";

interface MeshResource {
  readonly handle: MeshHandle;
  readonly vertexBuffer: WebGLBuffer;
  readonly indexBuffer: WebGLBuffer;
  readonly indexCount: number;
  readonly byteLength: number;
  readonly vertexArrays: Map<string, WebGLVertexArrayObject>;
  readonly bounds: MeshData["bounds"];
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

interface SceneRenderItem {
  readonly node: SceneNode;
  readonly model: Mat4;
  readonly basic: BasicMaterial | undefined;
  readonly depth: number;
}

interface SceneTexture {
  readonly handle: TextureHandle;
  readonly path: string;
  readonly cacheKey: string;
  readonly texture: WebGLTexture;
  readonly options: Required<TextureOptions>;
  readonly controller: AbortController;
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
  private readonly basicMaterials = new WeakSet<object>();
  private readonly meshHandles = new WeakMap<MeshHandle, MeshResource>();
  private readonly nodeHandles = new WeakMap<NodeHandle, SceneNode>();
  private readonly textureHandles = new WeakMap<object, SceneTexture>();
  private readonly pendingSetupAssets = new Set<Promise<void>>();
  private shaderRegistry: WebGL2ShaderRegistry;
  private basicProgram: BasicProgram | undefined;
  private startupPhase = true;
  private disposed = false;
  private meshBytes = 0;
  private drawCalls = 0;
  private triangles = 0;
  private culledNodes = 0;
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
    private readonly resourceLimits: RuntimeResourceLimits = {},
    private readonly textureFailurePolicy: TextureFailurePolicy = "stop",
    private readonly onTextureError: (
      reason: unknown,
      path: string,
    ) => void = () => {},
    private readonly stateCache = new WebGLStateCache(gl),
  ) {
    this.shaderRegistry = shaderRegistry;
  }

  public replaceShaderRegistry(registry: WebGL2ShaderRegistry): void {
    this.shaderRegistry = registry;
  }

  public stats(): SceneRendererStats {
    return Object.freeze({
      drawCalls: this.drawCalls,
      triangles: this.triangles,
      meshes: this.meshes.size,
      meshBytes: this.meshBytes,
      nodes: this.nodes.size,
      textures: this.textures.size,
      culledNodes: this.culledNodes,
    });
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
    }, this.owner, "material");
    this.basicMaterials.add(material);
    return Object.freeze(material) as BasicMaterial;
  }

  public nodeAdd(mesh: MeshHandle, material: MaterialHandle): NodeHandle {
    const resource = this.requireMesh(mesh);
    this.assertResourceCount(
      "scene nodes",
      this.nodes.size,
      this.resourceLimits.maxNodes,
    );
    if (this.basicMaterials.has(material)) {
      this.requireOwned(material as BasicMaterial, "material");
    } else if (!this.shaderRegistry.owns(material as ShaderMaterial)) {
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
    this.meshBytes -= resource.byteLength;
    for (const vertexArray of resource.vertexArrays.values()) {
      this.gl.deleteVertexArray(vertexArray);
    }
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

  public textureLoad(path: string, options?: TextureOptions): TextureHandle {
    const safePath = validateAssetPath(path);
    const safeOptions = validateTextureOptions(options);
    const cacheKey = textureCacheKey(safePath, safeOptions);
    const cached = this.textures.get(cacheKey);
    if (cached !== undefined) {
      return cached.handle;
    }
    this.assertResourceCount(
      "textures",
      this.textures.size,
      this.resourceLimits.maxTextures,
    );
    const texture = this.gl.createTexture();
    if (texture === null) {
      throw new Error(`failed to create texture for \`${safePath}\``);
    }
    this.stateCache.activateTexture(this.gl.TEXTURE0);
    this.stateCache.bindTexture2d(texture);
    this.applyTextureParameters(safeOptions);
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
      "texture",
    ) as TextureHandle;
    Object.freeze(handle);
    resource = {
      handle,
      path: safePath,
      cacheKey,
      texture,
      options: safeOptions,
      controller: new AbortController(),
      loaded: false,
      disposed: false,
      references: 0,
    };
    this.textures.set(cacheKey, resource);
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
    resource.controller.abort();
    this.textures.delete(resource.cacheKey);
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
    const knownTexture = typeof value === "object" && value !== null
      ? this.textureHandles.get(value as object)
      : undefined;
    if (knownTexture !== undefined) {
      textureResource = this.requireTexture(knownTexture.handle);
      uploadValue = textureResource.texture;
    } else if (
      typeof value === "object" &&
      value !== null &&
      !Array.isArray(value) &&
      !(value instanceof Float32Array)
    ) {
      throw new Error("invalid texture handle");
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
    this.stateCache.setDepthTest(true);
    this.stateCache.setDepthFunction(this.gl.LEQUAL);
    this.gl.clear(this.gl.DEPTH_BUFFER_BIT);
    const opaque: SceneRenderItem[] = [];
    const transparent: SceneRenderItem[] = [];
    for (const node of this.nodes) {
      const model = model4(node.position, node.rotation, node.scale);
      if (this.basicMaterials.has(node.material)) {
        if (this.basicNodeOutsideFrustum(node, model, view, width, height)) {
          this.culledNodes += 1;
          continue;
        }
        const material = node.material as BasicMaterial;
        const center = transformPoint3(
          view,
          transformPoint3(model, node.mesh.bounds.center),
        );
        const item = { node, model, basic: material, depth: -center[2] };
        (material.color[3] < 1 ? transparent : opaque).push(item);
        continue;
      }
      opaque.push({ node, model, basic: undefined, depth: 0 });
    }
    this.gl.depthMask(true);
    for (const item of opaque) {
      this.drawItem(item, view, projection, elapsedSeconds, width, height);
    }
    if (transparent.length > 0) {
      transparent.sort((left, right) => right.depth - left.depth);
      this.stateCache.enableBlend();
      this.gl.depthMask(false);
      for (const item of transparent) {
        this.drawItem(item, view, projection, elapsedSeconds, width, height);
      }
      this.gl.depthMask(true);
    }
    this.stateCache.bindVertexArray(null);
    this.stateCache.setDepthTest(false);
  }

  public dispose(): void {
    if (this.disposed) {
      return;
    }
    this.disposed = true;
    for (const mesh of this.meshes) {
      for (const vertexArray of mesh.vertexArrays.values()) {
        this.gl.deleteVertexArray(vertexArray);
      }
      this.gl.deleteBuffer(mesh.vertexBuffer);
      this.gl.deleteBuffer(mesh.indexBuffer);
    }
    for (const texture of this.textures.values()) {
      texture.disposed = true;
      texture.controller.abort();
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
    this.assertResourceCount(
      "meshes",
      this.meshes.size,
      this.resourceLimits.maxMeshes,
    );
    const byteLength = data.vertices.byteLength + data.indices.byteLength;
    const meshLimit = this.resourceLimits.maxMeshBytes;
    if (meshLimit !== undefined && this.meshBytes + byteLength > meshLimit) {
      throw new RangeError(
        `mesh byte budget exceeded: ${this.meshBytes + byteLength} > ${meshLimit}`,
      );
    }
    const vertexBuffer = this.gl.createBuffer();
    const indexBuffer = this.gl.createBuffer();
    if (vertexBuffer === null || indexBuffer === null) {
      if (vertexBuffer !== null) this.gl.deleteBuffer(vertexBuffer);
      if (indexBuffer !== null) this.gl.deleteBuffer(indexBuffer);
      throw new Error("failed to create mesh buffers");
    }
    this.stateCache.bindVertexArray(null);
    this.stateCache.bindArrayBuffer(vertexBuffer);
    this.gl.bufferData(this.gl.ARRAY_BUFFER, data.vertices, this.gl.STATIC_DRAW);
    this.stateCache.bindElementArrayBuffer(indexBuffer);
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
      byteLength,
      vertexArrays: new Map(),
      bounds: data.bounds,
      references: 0,
    };
    this.meshes.add(mesh);
    this.meshBytes += byteLength;
    this.meshHandles.set(handle, mesh);
    return handle;
  }

  private bindMesh(
    mesh: MeshResource,
    attributes: readonly ShaderAttribute[],
  ): void {
    const layoutKey = attributes
      .map((attribute) => `${attribute.name}:${attribute.location}`)
      .join("|");
    const cached = mesh.vertexArrays.get(layoutKey);
    if (cached !== undefined) {
      this.stateCache.bindVertexArray(cached);
      return;
    }
    const vertexArray = this.gl.createVertexArray();
    if (vertexArray === null) {
      throw new Error("failed to create a mesh vertex array");
    }
    this.stateCache.bindVertexArray(vertexArray);
    this.stateCache.bindArrayBuffer(mesh.vertexBuffer);
    this.stateCache.bindElementArrayBuffer(mesh.indexBuffer);
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
    mesh.vertexArrays.set(layoutKey, vertexArray);
  }

  private basicNodeOutsideFrustum(
    node: SceneNode,
    model: Mat4,
    view: Mat4,
    width: number,
    height: number,
  ): boolean {
    const worldCenter = transformPoint3(model, node.mesh.bounds.center);
    const center = transformPoint3(view, worldCenter);
    const radius = node.mesh.bounds.radius * Math.max(
      Math.abs(node.scale[0]),
      Math.abs(node.scale[1]),
      Math.abs(node.scale[2]),
    );
    const depth = -center[2];
    if (depth + radius < this.camera.near) return true;
    if (depth - radius > this.camera.far) return true;
    if (depth - radius <= this.camera.near) return false;
    const vertical = depth * Math.tan(this.camera.verticalFov / 2);
    const horizontal = vertical * (Math.max(1, width) / Math.max(1, height));
    return Math.abs(center[0]) > horizontal + radius ||
      Math.abs(center[1]) > vertical + radius;
  }

  private drawItem(
    item: SceneRenderItem,
    view: Mat4,
    projection: Mat4,
    elapsedSeconds: number,
    width: number,
    height: number,
  ): void {
    if (item.basic !== undefined) {
      this.bindBasic(item.basic, item.model, view, projection);
      this.bindMesh(item.node.mesh, STANDARD_ATTRIBUTES);
    } else {
      const attributes = this.shaderRegistry.bindForDraw(
        item.node.material as ShaderMaterial,
        item.node.uniforms,
        {
          elapsedSeconds,
          width,
          height,
          model: item.model,
          view,
          projection,
        },
      );
      this.bindMesh(item.node.mesh, attributes);
    }
    this.gl.drawElements(
      this.gl.TRIANGLES,
      item.node.mesh.indexCount,
      this.gl.UNSIGNED_INT,
      0,
    );
    this.drawCalls += 1;
    this.triangles += item.node.mesh.indexCount / 3;
  }

  private bindBasic(
    material: BasicMaterial,
    model: Mat4,
    view: Mat4,
    projection: Mat4,
  ): void {
    const basic = this.basicProgram ?? this.createBasicProgram();
    this.basicProgram = basic;
    this.stateCache.useProgram(basic.program);
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
      image = await this.imageLoader(url, {
        signal: handle.controller.signal,
      });
    } catch (error) {
      if (
        this.disposed ||
        handle.disposed ||
        handle.controller.signal.aborted ||
        this.textures.get(handle.cacheKey) !== handle
      ) {
        return;
      }
      const failure = new Error(
        `failed to load texture \`${handle.path}\`: ${errorMessage(error)}`,
      );
      if (this.textureFailurePolicy === "placeholder") {
        this.onTextureError(failure, handle.path);
        return;
      }
      throw failure;
    }
    if (
      this.disposed ||
      handle.disposed ||
      handle.controller.signal.aborted ||
      this.textures.get(handle.cacheKey) !== handle
    ) {
      return;
    }
    const dimensions = imageDimensions(image, handle.path);
    const dimensionLimit = this.textureDimensionLimit();
    if (
      dimensions.width > dimensionLimit ||
      dimensions.height > dimensionLimit
    ) {
      throw new RangeError(
        `texture \`${handle.path}\` is ${dimensions.width}x${dimensions.height}, exceeding the ${dimensionLimit}px texture limit`,
      );
    }
    this.stateCache.activateTexture(this.gl.TEXTURE0);
    this.stateCache.bindTexture2d(handle.texture);
    this.withTextureUnpackState(handle.options, () => {
      this.gl.texImage2D(
        this.gl.TEXTURE_2D,
        0,
        this.gl.RGBA,
        this.gl.RGBA,
        this.gl.UNSIGNED_BYTE,
        image,
      );
    });
    if (handle.options.mipmaps) {
      this.gl.generateMipmap(this.gl.TEXTURE_2D);
    }
    handle.loaded = true;
  }

  private applyTextureParameters(options: Required<TextureOptions>): void {
    const gl = this.gl;
    gl.texParameteri(
      gl.TEXTURE_2D,
      gl.TEXTURE_MIN_FILTER,
      textureMinFilter(gl, options.minFilter),
    );
    gl.texParameteri(
      gl.TEXTURE_2D,
      gl.TEXTURE_MAG_FILTER,
      options.magFilter === "nearest" ? gl.NEAREST : gl.LINEAR,
    );
    gl.texParameteri(
      gl.TEXTURE_2D,
      gl.TEXTURE_WRAP_S,
      textureWrap(gl, options.wrapS),
    );
    gl.texParameteri(
      gl.TEXTURE_2D,
      gl.TEXTURE_WRAP_T,
      textureWrap(gl, options.wrapT),
    );
  }

  private withTextureUnpackState(
    options: Required<TextureOptions>,
    upload: () => void,
  ): void {
    const gl = this.gl;
    const states = [
      [gl.UNPACK_FLIP_Y_WEBGL, options.flipY],
      [gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, options.premultiplyAlpha],
      [
        gl.UNPACK_COLORSPACE_CONVERSION_WEBGL,
        options.colorSpaceConversion === "none" ? gl.NONE : gl.BROWSER_DEFAULT_WEBGL,
      ],
    ] as const;
    const previous = states.map(([parameter]) => gl.getParameter(parameter));
    try {
      for (const [parameter, value] of states) gl.pixelStorei(parameter, value);
      upload();
    } finally {
      for (let index = 0; index < states.length; index += 1) {
        const state = states[index];
        if (state !== undefined) gl.pixelStorei(state[0], previous[index]);
      }
    }
  }

  private textureDimensionLimit(): number {
    const configured = this.resourceLimits.maxTextureDimension ?? Infinity;
    const reported: unknown = this.gl.getParameter(this.gl.MAX_TEXTURE_SIZE);
    if (
      typeof reported !== "number" ||
      !Number.isSafeInteger(reported) ||
      reported <= 0
    ) {
      throw new Error("WebGL returned an invalid maximum texture dimension");
    }
    return Math.min(configured, reported);
  }

  private assertResourceCount(
    label: string,
    current: number,
    maximum: number | undefined,
  ): void {
    if (maximum !== undefined && current >= maximum) {
      throw new RangeError(`${label} budget of ${maximum} has been reached`);
    }
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
      this.textures.get(resource.cacheKey) !== resource
    ) {
      throw new Error("texture handle is no longer valid");
    }
    return resource;
  }

  private requireOwned(
    handle: { readonly [sceneOwnerBrand]: object },
    kind: string,
  ): void {
    const info = sceneHandleInfo.get(handle);
    if (info?.owner !== this.owner) {
      if (info === undefined) throw new Error(`invalid ${kind} handle`);
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

function brand<T extends object>(
  value: T,
  owner: object,
  kind: string,
): T {
  Object.defineProperty(value, sceneOwnerBrand, { value: owner });
  sceneHandleInfo.set(value, { owner, kind });
  return value;
}

function opaqueHandle(kind: "mesh" | "node", owner: object): object {
  return Object.freeze(brand({ kind }, owner, kind));
}

function fixedVector(
  value: NumericSequence,
  length: number,
  label: string,
): readonly number[] {
  return copyFixedFiniteSequence(value, length, label);
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

function transformPoint3(matrix: Mat4, point: Vec3): Vec3 {
  return [
    (matrix[0] ?? 0) * point[0] +
    (matrix[4] ?? 0) * point[1] +
    (matrix[8] ?? 0) * point[2] +
    (matrix[12] ?? 0),
    (matrix[1] ?? 0) * point[0] +
    (matrix[5] ?? 0) * point[1] +
    (matrix[9] ?? 0) * point[2] +
    (matrix[13] ?? 0),
    (matrix[2] ?? 0) * point[0] +
    (matrix[6] ?? 0) * point[1] +
    (matrix[10] ?? 0) * point[2] +
    (matrix[14] ?? 0),
  ];
}

function validateAssetPath(path: unknown): string {
  if (
    typeof path !== "string" ||
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

const DEFAULT_TEXTURE_OPTIONS: Required<TextureOptions> = Object.freeze({
  minFilter: "linear",
  magFilter: "linear",
  wrapS: "clamp-to-edge",
  wrapT: "clamp-to-edge",
  mipmaps: false,
  flipY: false,
  premultiplyAlpha: false,
  colorSpaceConversion: "browser-default",
});

function validateTextureOptions(value: unknown): Required<TextureOptions> {
  if (value === undefined) return DEFAULT_TEXTURE_OPTIONS;
  if (typeof value !== "object" || value === null) {
    throw new TypeError("texture options must be a plain object");
  }
  const prototype: unknown = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new TypeError("texture options must not use a custom prototype");
  }
  const properties = new Map<string, unknown>();
  for (const key of Reflect.ownKeys(value)) {
    if (typeof key !== "string") {
      throw new TypeError("texture options must not contain symbol properties");
    }
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (descriptor === undefined || !("value" in descriptor)) {
      throw new TypeError(`texture options.${key} must be a data property`);
    }
    properties.set(key, descriptor.value);
  }
  const known = new Set([
    "minFilter",
    "magFilter",
    "wrapS",
    "wrapT",
    "mipmaps",
    "flipY",
    "premultiplyAlpha",
    "colorSpaceConversion",
  ]);
  for (const name of properties.keys()) {
    if (!known.has(name)) throw new TypeError(`unknown texture option \`${name}\``);
  }
  const minFilter = textureChoice(
    properties,
    "minFilter",
    [
      "nearest",
      "linear",
      "nearest-mipmap-nearest",
      "linear-mipmap-nearest",
      "nearest-mipmap-linear",
      "linear-mipmap-linear",
    ] as const,
    DEFAULT_TEXTURE_OPTIONS.minFilter,
  );
  const mipmaps = textureBoolean(
    properties,
    "mipmaps",
    DEFAULT_TEXTURE_OPTIONS.mipmaps,
  );
  if (minFilter.includes("mipmap") && !mipmaps) {
    throw new TypeError("texture options.mipmaps must be true for a mipmap minFilter");
  }
  return Object.freeze({
    minFilter,
    magFilter: textureChoice(
      properties,
      "magFilter",
      ["nearest", "linear"] as const,
      DEFAULT_TEXTURE_OPTIONS.magFilter,
    ),
    wrapS: textureChoice(
      properties,
      "wrapS",
      ["clamp-to-edge", "repeat", "mirrored-repeat"] as const,
      DEFAULT_TEXTURE_OPTIONS.wrapS,
    ),
    wrapT: textureChoice(
      properties,
      "wrapT",
      ["clamp-to-edge", "repeat", "mirrored-repeat"] as const,
      DEFAULT_TEXTURE_OPTIONS.wrapT,
    ),
    mipmaps,
    flipY: textureBoolean(properties, "flipY", DEFAULT_TEXTURE_OPTIONS.flipY),
    premultiplyAlpha: textureBoolean(
      properties,
      "premultiplyAlpha",
      DEFAULT_TEXTURE_OPTIONS.premultiplyAlpha,
    ),
    colorSpaceConversion: textureChoice(
      properties,
      "colorSpaceConversion",
      ["browser-default", "none"] as const,
      DEFAULT_TEXTURE_OPTIONS.colorSpaceConversion,
    ),
  });
}

function textureChoice<const T extends readonly string[]>(
  properties: ReadonlyMap<string, unknown>,
  name: string,
  choices: T,
  fallback: T[number],
): T[number] {
  const value = properties.get(name);
  if (value === undefined) return fallback;
  if (typeof value !== "string" || !choices.includes(value)) {
    throw new TypeError(
      `texture options.${name} must be ${choices.join(" or ")}`,
    );
  }
  return value;
}

function textureBoolean(
  properties: ReadonlyMap<string, unknown>,
  name: string,
  fallback: boolean,
): boolean {
  const value = properties.get(name);
  if (value === undefined) return fallback;
  if (typeof value !== "boolean") {
    throw new TypeError(`texture options.${name} must be a boolean`);
  }
  return value;
}

function textureCacheKey(
  path: string,
  options: Required<TextureOptions>,
): string {
  return [
    path,
    options.minFilter,
    options.magFilter,
    options.wrapS,
    options.wrapT,
    options.mipmaps ? "1" : "0",
    options.flipY ? "1" : "0",
    options.premultiplyAlpha ? "1" : "0",
    options.colorSpaceConversion,
  ].join("\u0000");
}

function textureMinFilter(
  gl: WebGL2RenderingContext,
  value: Required<TextureOptions>["minFilter"],
): number {
  switch (value) {
    case "nearest": return gl.NEAREST;
    case "linear": return gl.LINEAR;
    case "nearest-mipmap-nearest": return gl.NEAREST_MIPMAP_NEAREST;
    case "linear-mipmap-nearest": return gl.LINEAR_MIPMAP_NEAREST;
    case "nearest-mipmap-linear": return gl.NEAREST_MIPMAP_LINEAR;
    case "linear-mipmap-linear": return gl.LINEAR_MIPMAP_LINEAR;
  }
}

function textureWrap(
  gl: WebGL2RenderingContext,
  value: Required<TextureOptions>["wrapS"],
): number {
  switch (value) {
    case "clamp-to-edge": return gl.CLAMP_TO_EDGE;
    case "repeat": return gl.REPEAT;
    case "mirrored-repeat": return gl.MIRRORED_REPEAT;
  }
}

function imageDimensions(
  source: unknown,
  path: string,
): { readonly width: number; readonly height: number } {
  if (typeof source !== "object" || source === null) {
    throw new TypeError(`image loader returned an invalid source for \`${path}\``);
  }
  const width = firstPositiveDimension(source, [
    "displayWidth",
    "videoWidth",
    "naturalWidth",
    "width",
  ]);
  const height = firstPositiveDimension(source, [
    "displayHeight",
    "videoHeight",
    "naturalHeight",
    "height",
  ]);
  if (width === undefined || height === undefined) {
    throw new TypeError(
      `image loader returned a source without positive dimensions for \`${path}\``,
    );
  }
  return { width, height };
}

function firstPositiveDimension(
  source: object,
  names: readonly string[],
): number | undefined {
  const prototype: unknown = Object.getPrototypeOf(source);
  const plain = prototype === Object.prototype || prototype === null;
  for (const name of names) {
    let value: unknown;
    if (plain) {
      const descriptor = Object.getOwnPropertyDescriptor(source, name);
      if (descriptor !== undefined && !("value" in descriptor)) {
        throw new TypeError(`image source.${name} must be a data property`);
      }
      value = descriptor?.value;
    } else {
      try {
        value = Reflect.get(source, name);
      } catch (error) {
        throw new TypeError(
          `image source.${name} could not be read: ${errorMessage(error)}`,
        );
      }
    }
    if (typeof value === "number" && Number.isSafeInteger(value) && value > 0) {
      return value;
    }
  }
  return undefined;
}

async function defaultImageLoader(
  url: string,
  request: RuntimeImageRequest,
): Promise<TexImageSource> {
  if (typeof globalThis.Image !== "function") {
    throw new Error("image loading is unavailable outside a browser");
  }
  return await new Promise<HTMLImageElement>((resolve, reject) => {
    const image = new globalThis.Image();
    const cleanup = (): void => {
      image.removeEventListener("load", handleLoad);
      image.removeEventListener("error", handleError);
      request.signal.removeEventListener("abort", handleAbort);
    };
    const handleLoad = (): void => {
      cleanup();
      resolve(image);
    };
    const handleError = (): void => {
      cleanup();
      reject(new Error(`browser could not decode ${url}`));
    };
    const handleAbort = (): void => {
      cleanup();
      image.src = "";
      const error = new Error(`image load aborted for ${url}`);
      error.name = "AbortError";
      reject(error);
    };
    if (request.signal.aborted) {
      handleAbort();
      return;
    }
    image.addEventListener("load", handleLoad);
    image.addEventListener("error", handleError);
    request.signal.addEventListener("abort", handleAbort, { once: true });
    image.src = url;
  });
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
