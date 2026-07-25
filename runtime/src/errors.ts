export interface SourceLocation {
  readonly source: string;
  readonly line: number;
  readonly column: number;
  readonly start: number;
  readonly end: number;
}

export interface LocatedError extends Error {
  polyglLocation?: SourceLocation;
}

const OVERLAY_ID = "polygl-error-overlay";

export function runtimeError(
  message: string,
  location?: SourceLocation,
): LocatedError {
  const error = new Error(message) as LocatedError;
  if (location !== undefined) {
    error.polyglLocation = location;
  }
  return error;
}

export function formatRuntimeError(reason: unknown): string {
  const error = normalizeError(reason);
  const location = sourceLocation(error);
  if (location === undefined) {
    return error.message;
  }
  return `${location.source}:${location.line}:${location.column}: ${error.message}`;
}

export function showRuntimeError(
  reason: unknown,
  documentObject: Document | undefined = globalThis.document,
): void {
  if (documentObject === undefined) {
    return;
  }

  let overlay = documentObject.getElementById(OVERLAY_ID);
  if (overlay === null) {
    overlay = documentObject.createElement("pre");
    overlay.id = OVERLAY_ID;
    overlay.setAttribute("role", "alert");
    Object.assign(overlay.style, {
      background: "rgba(24, 8, 12, 0.96)",
      border: "1px solid #ff6b81",
      boxSizing: "border-box",
      color: "#fff1f3",
      font: "13px/1.5 ui-monospace, SFMono-Regular, Menlo, monospace",
      inset: "0",
      margin: "0",
      overflow: "auto",
      padding: "20px",
      position: "fixed",
      whiteSpace: "pre-wrap",
      zIndex: "2147483647",
    });
    documentObject.body.append(overlay);
  }

  const error = normalizeError(reason);
  const stack = error.stack;
  overlay.textContent =
    stack === undefined
      ? formatRuntimeError(error)
      : `${formatRuntimeError(error)}\n\n${stack}`;
}

function normalizeError(reason: unknown): Error {
  if (reason instanceof Error) {
    return reason;
  }
  return new Error(String(reason));
}

function sourceLocation(error: Error): SourceLocation | undefined {
  const candidate = (error as LocatedError).polyglLocation;
  if (
    candidate === undefined ||
    typeof candidate.source !== "string" ||
    !Number.isInteger(candidate.line) ||
    !Number.isInteger(candidate.column)
  ) {
    return undefined;
  }
  return candidate;
}
