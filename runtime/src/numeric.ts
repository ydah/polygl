const MAX_NUMERIC_SEQUENCE_LENGTH = 16_777_216;

export function copyDenseNumericSequence(
  value: unknown,
  label: string,
  maximumLength = MAX_NUMERIC_SEQUENCE_LENGTH,
): readonly number[] {
  if (!Number.isSafeInteger(maximumLength) || maximumLength < 0) {
    throw new RangeError("numeric sequence maximum length must be non-negative");
  }
  if (Array.isArray(value)) {
    if (Object.getPrototypeOf(value) !== Array.prototype) {
      throw new TypeError(`${label} must not use a custom array prototype`);
    }
    if (value.length > maximumLength) {
      throw new RangeError(`${label} cannot contain more than ${maximumLength} values`);
    }
    const result: number[] = [];
    for (let index = 0; index < value.length; index += 1) {
      const descriptor = Object.getOwnPropertyDescriptor(value, String(index));
      if (descriptor === undefined || !("value" in descriptor)) {
        throw new TypeError(`${label}[${index}] must be a data property`);
      }
      if (typeof descriptor.value !== "number") {
        throw new TypeError(`${label}[${index}] must be a number`);
      }
      result.push(descriptor.value);
    }
    for (const key of Reflect.ownKeys(value)) {
      if (key === "length") continue;
      if (typeof key !== "string" || !isDenseIndex(key, value.length)) {
        throw new TypeError(`${label} must contain only indexed data properties`);
      }
    }
    return result;
  }
  if (
    value instanceof Float32Array &&
    Object.getPrototypeOf(value) === Float32Array.prototype
  ) {
    if (value.length > maximumLength) {
      throw new RangeError(`${label} cannot contain more than ${maximumLength} values`);
    }
    const result: number[] = [];
    for (let index = 0; index < value.length; index += 1) {
      result.push(value[index] ?? 0);
    }
    return result;
  }
  throw new TypeError(`${label} must be an array or Float32Array`);
}

export function copyFixedFiniteSequence(
  value: unknown,
  length: number,
  label: string,
): readonly number[] {
  const components = copyDenseNumericSequence(value, label, length);
  if (
    components.length !== length ||
    components.some((item) => !Number.isFinite(item))
  ) {
    throw new RangeError(`${label} must contain ${length} finite numbers`);
  }
  return Object.freeze(components);
}

function isDenseIndex(key: string, length: number): boolean {
  if (!/^(0|[1-9][0-9]*)$/.test(key)) return false;
  const index = Number(key);
  return Number.isSafeInteger(index) && index >= 0 && index < length;
}
