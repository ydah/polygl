import assert from "node:assert/strict";
import {
  cp,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { PLATFORM_PACKAGES } from "../lib/platform.mjs";
import {
  NATIVE_PACKAGES,
  prepareRelease,
} from "../../scripts/prepare-release.mjs";

test("launcher, release staging, and native package manifests stay aligned", async () => {
  const launcher = JSON.parse(
    await readFile(new URL("../package.json", import.meta.url), "utf8"),
  );
  const resolvedPackages = new Set(Object.values(PLATFORM_PACKAGES));
  const stagedPackages = new Set(NATIVE_PACKAGES.map(({ name }) => name));
  const optionalPackages = new Set(Object.keys(launcher.optionalDependencies));

  assert.deepEqual(stagedPackages, resolvedPackages);
  assert.deepEqual(optionalPackages, resolvedPackages);

  for (const { directory, name } of NATIVE_PACKAGES) {
    const manifest = JSON.parse(
      await readFile(
        new URL(`../../platforms/${directory}/package.json`, import.meta.url),
        "utf8",
      ),
    );
    assert.equal(manifest.name, name);
  }
});

test("release preparation stages binaries and synchronizes versions", async () => {
  const temporary = await mkdtemp(join(tmpdir(), "polygl-npm-release-"));
  const packagesRoot = join(temporary, "npm");
  const artifactsRoot = join(temporary, "artifacts");
  try {
    await cp(new URL("../..", import.meta.url), packagesRoot, {
      recursive: true,
    });
    for (const { directory, binary } of NATIVE_PACKAGES) {
      const binaryDirectory = join(artifactsRoot, "bundle", directory);
      await mkdir(binaryDirectory, { recursive: true });
      await writeFile(join(binaryDirectory, binary), `${directory}\n`);
    }

    await prepareRelease("1.2.3-beta.1", artifactsRoot, packagesRoot);

    const launcher = JSON.parse(
      await readFile(join(packagesRoot, "cli", "package.json"), "utf8"),
    );
    assert.equal(launcher.version, "1.2.3-beta.1");
    assert.ok(
      Object.values(launcher.optionalDependencies).every(
        (version) => version === "1.2.3-beta.1",
      ),
    );
    for (const { directory, binary } of NATIVE_PACKAGES) {
      const packageRoot = join(packagesRoot, "platforms", directory);
      const manifest = JSON.parse(
        await readFile(join(packageRoot, "package.json"), "utf8"),
      );
      assert.equal(manifest.version, "1.2.3-beta.1");
      assert.equal(
        await readFile(join(packageRoot, "bin", binary), "utf8"),
        `${directory}\n`,
      );
      await assert.rejects(stat(join(packageRoot, "bin", ".gitkeep")));
    }
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
});
