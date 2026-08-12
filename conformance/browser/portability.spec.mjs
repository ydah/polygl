import { expect, test } from "@playwright/test";
import { spawnSync } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const workspaceRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
const executable = path.join(workspaceRoot, "target", "debug", "polygl");
let buildRoot;

test.beforeAll(async ({}, testInfo) => {
  testInfo.setTimeout(180_000);
  run("cargo", ["build", "--quiet", "-p", "polygl-cli"]);
  buildRoot = await mkdtemp(path.join(tmpdir(), "polygl-portability-"));
  run(executable, [
    "build",
    path.join(workspaceRoot, "conformance", "cases", "rectangle", "main.rb"),
    "-o",
    buildRoot,
    "--release",
  ]);
});

test.afterAll(async () => {
  if (buildRoot !== undefined) {
    await rm(buildRoot, { recursive: true, force: true });
  }
});

test("loads a generated WebGL2 artifact without a runtime error", async ({
  browserName,
  page,
}) => {
  await page.route("http://polygl-portability.test/**", async (route) => {
    const url = new URL(route.request().url());
    const file = path.resolve(buildRoot, `.${decodeURIComponent(url.pathname)}`);
    if (!file.startsWith(`${path.resolve(buildRoot)}${path.sep}`)) {
      await route.fulfill({ status: 403, body: "forbidden" });
      return;
    }
    try {
      await route.fulfill({
        status: 200,
        body: await readFile(file),
        contentType: contentType(file),
      });
    } catch {
      await route.fulfill({ status: 404, body: "not found" });
    }
  });
  await page.goto("http://polygl-portability.test/index.html");
  await page.evaluate(() => globalThis.__polyglReady);
  const result = await page.evaluate(() => {
    const canvas = document.querySelector("canvas");
    const gl = canvas?.getContext("webgl2");
    return {
      context: gl !== null,
      error: document.getElementById("polygl-error-overlay")?.textContent,
      dimensions: canvas === null ? [] : [canvas.width, canvas.height],
    };
  });
  expect(result.error, `${browserName} displayed a runtime error`).toBeUndefined();
  expect(result.context, `${browserName} did not provide WebGL2`).toBe(true);
  expect(result.dimensions).toEqual([48, 32]);
});

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: workspaceRoot,
    encoding: "utf8",
  });
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} failed\n${result.stdout}\n${result.stderr}`,
    );
  }
}

function contentType(file) {
  switch (path.extname(file)) {
    case ".html":
      return "text/html; charset=utf-8";
    case ".js":
      return "text/javascript; charset=utf-8";
    default:
      return "application/octet-stream";
  }
}
