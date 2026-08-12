import { runtimeError } from "./errors.js";
import type { SourceLocation } from "./errors.js";

export type PolyglRecord<T> = { [key: string]: T };

export function mapFromEntries<T>(
  entries: ReadonlyArray<readonly [string, T]>,
): PolyglRecord<T> {
  return recordFromEntries(entries);
}

export function structFromEntries(
  entries: ReadonlyArray<readonly [string, unknown]>,
): PolyglRecord<unknown> {
  return recordFromEntries(entries);
}

export function mapGet<T>(
  collection: PolyglRecord<T> | null | undefined,
  key: string,
  location?: SourceLocation,
): T {
  if (collection === null || collection === undefined) {
    throw runtimeError("cannot index nil", location);
  }
  if (!Object.prototype.hasOwnProperty.call(collection, key)) {
    throw runtimeError(`map key ${JSON.stringify(key)} is not present`, location);
  }
  return collection[key] as T;
}

export function mapSet<T>(
  collection: PolyglRecord<T> | null | undefined,
  key: string,
  value: T,
  location?: SourceLocation,
): T {
  if (collection === null || collection === undefined) {
    throw runtimeError("cannot index nil", location);
  }
  defineEntry(collection, key, value);
  return value;
}

function recordFromEntries<T>(
  entries: ReadonlyArray<readonly [string, T]>,
): PolyglRecord<T> {
  const record = Object.create(null) as PolyglRecord<T>;
  for (const [key, value] of entries) {
    defineEntry(record, key, value);
  }
  return record;
}

function defineEntry<T>(record: PolyglRecord<T>, key: string, value: T): void {
  Object.defineProperty(record, key, {
    configurable: true,
    enumerable: true,
    value,
    writable: true,
  });
}
