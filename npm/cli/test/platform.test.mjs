import assert from "node:assert/strict";
import test from "node:test";

import {
  PLATFORM_PACKAGES,
  binaryName,
  resolveBinary,
  selectPackage,
} from "../lib/platform.mjs";

test("selects every published platform package", () => {
  assert.equal(selectPackage("linux", "x64"), "@polygl/cli-linux-x64");
  assert.equal(selectPackage("linux", "arm64"), "@polygl/cli-linux-arm64");
  assert.equal(selectPackage("darwin", "x64"), "@polygl/cli-darwin-x64");
  assert.equal(selectPackage("darwin", "arm64"), "@polygl/cli-darwin-arm64");
  assert.equal(selectPackage("win32", "x64"), "@polygl/cli-win32-x64");
  assert.equal(Object.keys(PLATFORM_PACKAGES).length, 5);
});

test("uses the Windows executable suffix only on Windows", () => {
  assert.equal(binaryName("linux"), "polygl");
  assert.equal(binaryName("darwin"), "polygl");
  assert.equal(binaryName("win32"), "polygl.exe");
});

test("reports unsupported and missing platform packages", () => {
  assert.throws(() => selectPackage("freebsd", "x64"), /unsupported platform freebsd-x64/);
  assert.throws(
    () =>
      resolveBinary(
        () => {
          throw new Error("not installed");
        },
        "linux",
        "x64",
      ),
    /native package @polygl\/cli-linux-x64 is unavailable/,
  );
});

test("resolves the binary inside the selected optional dependency", () => {
  const requests = [];
  const resolved = resolveBinary(
    (request) => {
      requests.push(request);
      return `/packages/${request}`;
    },
    "win32",
    "x64",
  );
  assert.deepEqual(requests, ["@polygl/cli-win32-x64/bin/polygl.exe"]);
  assert.equal(resolved, "/packages/@polygl/cli-win32-x64/bin/polygl.exe");
});
