export type Vec3 = readonly [number, number, number];
export type Mat4 = Float32Array;

export function identity4(): Mat4 {
  return new Float32Array([
    1, 0, 0, 0,
    0, 1, 0, 0,
    0, 0, 1, 0,
    0, 0, 0, 1,
  ]);
}

export function perspective4(
  verticalFov: number,
  aspect: number,
  near: number,
  far: number,
): Mat4 {
  const scale = 1 / Math.tan(verticalFov / 2);
  const depth = 1 / (near - far);
  return new Float32Array([
    scale / aspect, 0, 0, 0,
    0, scale, 0, 0,
    0, 0, (far + near) * depth, -1,
    0, 0, 2 * far * near * depth, 0,
  ]);
}

export function lookAt4(eye: Vec3, target: Vec3, up: Vec3): Mat4 {
  const z = normalize3(subtract3(eye, target), "camera eye and target");
  const x = normalize3(cross3(up, z), "camera up direction");
  const y = cross3(z, x);
  return new Float32Array([
    x[0], y[0], z[0], 0,
    x[1], y[1], z[1], 0,
    x[2], y[2], z[2], 0,
    -dot3(x, eye), -dot3(y, eye), -dot3(z, eye), 1,
  ]);
}

export function model4(
  position: Vec3,
  rotation: Vec3,
  scale: Vec3,
): Mat4 {
  const [rx, ry, rz] = rotation;
  const sx = Math.sin(rx);
  const cx = Math.cos(rx);
  const sy = Math.sin(ry);
  const cy = Math.cos(ry);
  const sz = Math.sin(rz);
  const cz = Math.cos(rz);

  const rotationX = new Float32Array([
    1, 0, 0, 0,
    0, cx, sx, 0,
    0, -sx, cx, 0,
    0, 0, 0, 1,
  ]);
  const rotationY = new Float32Array([
    cy, 0, -sy, 0,
    0, 1, 0, 0,
    sy, 0, cy, 0,
    0, 0, 0, 1,
  ]);
  const rotationZ = new Float32Array([
    cz, sz, 0, 0,
    -sz, cz, 0, 0,
    0, 0, 1, 0,
    0, 0, 0, 1,
  ]);
  const scaling = new Float32Array([
    scale[0], 0, 0, 0,
    0, scale[1], 0, 0,
    0, 0, scale[2], 0,
    0, 0, 0, 1,
  ]);
  const translation = identity4();
  translation[12] = position[0];
  translation[13] = position[1];
  translation[14] = position[2];
  return multiply4(
    translation,
    multiply4(
      rotationZ,
      multiply4(rotationY, multiply4(rotationX, scaling)),
    ),
  );
}

export function normal3(model: Mat4): Float32Array {
  const a00 = model[0] ?? 0;
  const a01 = model[1] ?? 0;
  const a02 = model[2] ?? 0;
  const a10 = model[4] ?? 0;
  const a11 = model[5] ?? 0;
  const a12 = model[6] ?? 0;
  const a20 = model[8] ?? 0;
  const a21 = model[9] ?? 0;
  const a22 = model[10] ?? 0;
  const determinant =
    a00 * (a11 * a22 - a12 * a21) -
    a10 * (a01 * a22 - a02 * a21) +
    a20 * (a01 * a12 - a02 * a11);
  if (!Number.isFinite(determinant) || Math.abs(determinant) < 1e-12) {
    throw new RangeError("node scale must produce an invertible model matrix");
  }
  const inverse = 1 / determinant;
  return new Float32Array([
    (a11 * a22 - a12 * a21) * inverse,
    (a12 * a20 - a10 * a22) * inverse,
    (a10 * a21 - a11 * a20) * inverse,
    (a02 * a21 - a01 * a22) * inverse,
    (a00 * a22 - a02 * a20) * inverse,
    (a01 * a20 - a00 * a21) * inverse,
    (a01 * a12 - a02 * a11) * inverse,
    (a02 * a10 - a00 * a12) * inverse,
    (a00 * a11 - a01 * a10) * inverse,
  ]);
}

export function normalize3(value: Vec3, label = "vector"): Vec3 {
  const length = Math.hypot(...value);
  if (!Number.isFinite(length) || length <= 1e-12) {
    throw new RangeError(`${label} must not be zero`);
  }
  return [value[0] / length, value[1] / length, value[2] / length];
}

function multiply4(left: Mat4, right: Mat4): Mat4 {
  const result = new Float32Array(16);
  for (let column = 0; column < 4; column += 1) {
    for (let row = 0; row < 4; row += 1) {
      let value = 0;
      for (let index = 0; index < 4; index += 1) {
        value +=
          (left[index * 4 + row] ?? 0) *
          (right[column * 4 + index] ?? 0);
      }
      result[column * 4 + row] = value;
    }
  }
  return result;
}

function subtract3(left: Vec3, right: Vec3): Vec3 {
  return [
    left[0] - right[0],
    left[1] - right[1],
    left[2] - right[2],
  ];
}

function cross3(left: Vec3, right: Vec3): Vec3 {
  return [
    left[1] * right[2] - left[2] * right[1],
    left[2] * right[0] - left[0] * right[2],
    left[0] * right[1] - left[1] * right[0],
  ];
}

function dot3(left: Vec3, right: Vec3): number {
  return left[0] * right[0] + left[1] * right[1] + left[2] * right[2];
}
