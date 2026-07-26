export const FLOATS_PER_MESH_VERTEX = 12;

export interface MeshData {
  readonly vertices: Float32Array;
  readonly indices: Uint32Array;
}

export function boxMesh(width: number, height: number, depth: number): MeshData {
  const x = positive(width, "box width") / 2;
  const y = positive(height, "box height") / 2;
  const z = positive(depth, "box depth") / 2;
  const vertices: number[] = [];
  const indices: number[] = [];
  const faces = [
    [[0, 0, 1], [-x, -y, z], [x, -y, z], [x, y, z], [-x, y, z]],
    [[0, 0, -1], [x, -y, -z], [-x, -y, -z], [-x, y, -z], [x, y, -z]],
    [[1, 0, 0], [x, -y, z], [x, -y, -z], [x, y, -z], [x, y, z]],
    [[-1, 0, 0], [-x, -y, -z], [-x, -y, z], [-x, y, z], [-x, y, -z]],
    [[0, 1, 0], [-x, y, z], [x, y, z], [x, y, -z], [-x, y, -z]],
    [[0, -1, 0], [-x, -y, -z], [x, -y, -z], [x, -y, z], [-x, -y, z]],
  ] as const;
  const uvs = [[0, 0], [1, 0], [1, 1], [0, 1]] as const;
  for (const [normal, ...positions] of faces) {
    const start = vertices.length / FLOATS_PER_MESH_VERTEX;
    for (let index = 0; index < positions.length; index += 1) {
      pushVertex(vertices, positions[index]!, normal, uvs[index]!);
    }
    indices.push(start, start + 1, start + 2, start, start + 2, start + 3);
  }
  return createMeshData(vertices, indices);
}

export function sphereMesh(radius: number, segments: number): MeshData {
  const safeRadius = positive(radius, "sphere radius");
  const columns = boundedSegments(segments, "sphere segments");
  const rows = Math.max(2, Math.ceil(columns / 2));
  const vertices: number[] = [];
  const indices: number[] = [];
  for (let row = 0; row <= rows; row += 1) {
    const v = row / rows;
    const latitude = v * Math.PI;
    const y = Math.cos(latitude);
    const ring = Math.sin(latitude);
    for (let column = 0; column <= columns; column += 1) {
      const u = column / columns;
      const longitude = u * Math.PI * 2;
      const normal = [
        ring * Math.cos(longitude),
        y,
        ring * Math.sin(longitude),
      ] as const;
      pushVertex(
        vertices,
        [
          normal[0] * safeRadius,
          normal[1] * safeRadius,
          normal[2] * safeRadius,
        ],
        normal,
        [u, 1 - v],
      );
    }
  }
  for (let row = 0; row < rows; row += 1) {
    for (let column = 0; column < columns; column += 1) {
      const topLeft = row * (columns + 1) + column;
      const bottomLeft = topLeft + columns + 1;
      indices.push(
        topLeft,
        bottomLeft,
        topLeft + 1,
        topLeft + 1,
        bottomLeft,
        bottomLeft + 1,
      );
    }
  }
  return createMeshData(vertices, indices);
}

export function planeMesh(
  width: number,
  depth: number,
  columns = 1,
  rows = 1,
): MeshData {
  const safeWidth = positive(width, "plane width");
  const safeDepth = positive(depth, "plane depth");
  const safeColumns = boundedSegments(columns, "plane columns", 1);
  const safeRows = boundedSegments(rows, "plane rows", 1);
  const vertices: number[] = [];
  const indices: number[] = [];
  for (let row = 0; row <= safeRows; row += 1) {
    const v = row / safeRows;
    for (let column = 0; column <= safeColumns; column += 1) {
      const u = column / safeColumns;
      pushVertex(
        vertices,
        [(u - 0.5) * safeWidth, 0, (v - 0.5) * safeDepth],
        [0, 1, 0],
        [u, v],
      );
    }
  }
  for (let row = 0; row < safeRows; row += 1) {
    for (let column = 0; column < safeColumns; column += 1) {
      const topLeft = row * (safeColumns + 1) + column;
      const bottomLeft = topLeft + safeColumns + 1;
      indices.push(
        topLeft,
        topLeft + 1,
        bottomLeft,
        topLeft + 1,
        bottomLeft + 1,
        bottomLeft,
      );
    }
  }
  return createMeshData(vertices, indices);
}

export function customMesh(
  vertices: readonly number[],
  indices: readonly number[],
): MeshData {
  if (
    vertices.length === 0 ||
    vertices.length % FLOATS_PER_MESH_VERTEX !== 0
  ) {
    throw new RangeError(
      `mesh vertices must contain ${FLOATS_PER_MESH_VERTEX} finite values per vertex`,
    );
  }
  if (vertices.some((value) => !Number.isFinite(value))) {
    throw new RangeError("mesh vertices must be finite numbers");
  }
  const vertexCount = vertices.length / FLOATS_PER_MESH_VERTEX;
  if (
    indices.length === 0 ||
    indices.length % 3 !== 0 ||
    indices.some(
      (value) =>
        !Number.isInteger(value) ||
        value < 0 ||
        value >= vertexCount ||
        value > 0xffff_ffff,
    )
  ) {
    throw new RangeError(
      "mesh indices must be in-range unsigned triangle indices",
    );
  }
  return createMeshData(vertices, indices);
}

function createMeshData(
  vertices: readonly number[],
  indices: readonly number[],
): MeshData {
  return {
    vertices: new Float32Array(vertices),
    indices: new Uint32Array(indices),
  };
}

function pushVertex(
  target: number[],
  position: readonly [number, number, number],
  normal: readonly [number, number, number],
  uv: readonly [number, number],
): void {
  target.push(...position, ...normal, ...uv, 1, 1, 1, 1);
}

function positive(value: number, label: string): number {
  if (!Number.isFinite(value) || value <= 0) {
    throw new RangeError(`${label} must be a positive finite number`);
  }
  return value;
}

function boundedSegments(value: number, label: string, minimum = 3): number {
  if (!Number.isInteger(value) || value < minimum || value > 512) {
    throw new RangeError(
      `${label} must be an integer between ${minimum} and 512`,
    );
  }
  return value;
}
