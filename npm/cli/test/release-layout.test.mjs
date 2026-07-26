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
  LEGAL_FILES,
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
  assert.ok(LEGAL_FILES.every((file) => launcher.files.includes(file)));

  for (const { directory, name } of NATIVE_PACKAGES) {
    const manifest = JSON.parse(
      await readFile(
        new URL(`../../platforms/${directory}/package.json`, import.meta.url),
        "utf8",
      ),
    );
    assert.equal(manifest.name, name);
    assert.ok(LEGAL_FILES.every((file) => manifest.files.includes(file)));
  }
});

test("release preparation stages binaries and synchronizes versions", async () => {
  const temporary = await mkdtemp(join(tmpdir(), "polygl-npm-release-"));
  const packagesRoot = join(temporary, "npm");
  const artifactsRoot = join(temporary, "artifacts");
  const legalRoot = join(temporary, "legal");
  try {
    await cp(new URL("../..", import.meta.url), packagesRoot, {
      recursive: true,
    });
    await mkdir(legalRoot, { recursive: true });
    for (const file of LEGAL_FILES) {
      await writeFile(join(legalRoot, file), `${file}\n`);
    }
    for (const { directory, binary } of NATIVE_PACKAGES) {
      const binaryDirectory = join(artifactsRoot, "bundle", directory);
      await mkdir(binaryDirectory, { recursive: true });
      await writeFile(join(binaryDirectory, binary), `${directory}\n`);
    }

    await prepareRelease(
      "1.2.3-beta.1",
      artifactsRoot,
      packagesRoot,
      legalRoot,
    );

    const launcher = JSON.parse(
      await readFile(join(packagesRoot, "cli", "package.json"), "utf8"),
    );
    assert.equal(launcher.version, "1.2.3-beta.1");
    assert.ok(
      Object.values(launcher.optionalDependencies).every(
        (version) => version === "1.2.3-beta.1",
      ),
    );
    for (const file of LEGAL_FILES) {
      assert.equal(
        await readFile(join(packagesRoot, "cli", file), "utf8"),
        `${file}\n`,
      );
    }
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
      for (const file of LEGAL_FILES) {
        assert.equal(
          await readFile(join(packageRoot, file), "utf8"),
          `${file}\n`,
        );
      }
      await assert.rejects(stat(join(packageRoot, "bin", ".gitkeep")));
    }
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
});
