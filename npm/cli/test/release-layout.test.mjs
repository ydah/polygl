import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import {
  cp,
  mkdir,
  mkdtemp,
  readFile,
  rename,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { promisify } from "node:util";

import { PLATFORM_PACKAGES } from "../lib/platform.mjs";
import {
  LEGAL_FILES,
  NATIVE_PACKAGES,
  prepareRelease,
} from "../../scripts/prepare-release.mjs";
import {
  PACKED_PACKAGES,
  validateInstallabilityMetadata,
  verifyPackedRelease,
} from "../../scripts/verify-packed-release.mjs";

const execFileAsync = promisify(execFile);

async function npmPack(arguments_, options) {
  const version = process.env.POLYGL_TEST_NPM_VERSION;
  if (version) {
    return execFileAsync(
      "npx",
      ["--yes", `npm@${version}`, "pack", ...arguments_],
      options,
    );
  }
  return execFileAsync("npm", ["pack", ...arguments_], options);
}

test("release workflow packs only explicit local package paths", async () => {
  const workflow = await readFile(
    new URL("../../../.github/workflows/release.yml", import.meta.url),
    "utf8",
  );
  const block = workflow.match(/packages=\(\n([\s\S]*?)\n\s+\)/)?.[1];
  assert.ok(block);
  const specs = block.trim().split("\n").map((line) => line.trim());
  assert.deepEqual(specs, [
    "./npm/platforms/darwin-arm64:polygl-cli-darwin-arm64",
    "./npm/platforms/darwin-x64:polygl-cli-darwin-x64",
    "./npm/platforms/linux-arm64:polygl-cli-linux-arm64",
    "./npm/platforms/linux-x64:polygl-cli-linux-x64",
    "./npm/platforms/win32-x64:polygl-cli-win32-x64",
    "./npm/cli:polygl-cli",
  ]);
  assert.ok(specs.every((spec) => spec.startsWith("./npm/")));
});

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

test("packed package metadata remains installable", () => {
  const native = PACKED_PACKAGES.find(
    ({ name }) => name === "@polygl/cli-linux-x64",
  );
  assert.throws(
    () => validateInstallabilityMetadata(
      { os: ["darwin"], cpu: ["x64"] },
      native,
    ),
    /must target only linux/,
  );
  const launcher = PACKED_PACKAGES.find(
    ({ name }) => name === "@polygl/cli",
  );
  assert.throws(
    () => validateInstallabilityMetadata(
      { bin: { polygl: "bin/not-polygl.js" } },
      launcher,
    ),
    /bin\.polygl/,
  );
});

test("release preparation stages binaries and synchronizes versions", async () => {
  const temporary = await mkdtemp(join(tmpdir(), "polygl-npm-release-"));
  const packagesRoot = join(temporary, "npm");
  const artifactsRoot = join(temporary, "artifacts");
  const legalRoot = join(temporary, "legal");
  const packedRoot = join(temporary, "packed");
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

    await mkdir(packedRoot);
    for (const { directory } of NATIVE_PACKAGES) {
      const { stdout } = await npmPack(
        [
          join(packagesRoot, "platforms", directory),
          "--ignore-scripts",
          "--pack-destination",
          packedRoot,
        ],
        {
          env: {
            ...process.env,
            npm_config_cache: join(temporary, "npm-cache"),
          },
        },
      );
      await rename(
        join(packedRoot, stdout.trim().split("\n").at(-1)),
        join(packedRoot, `polygl-cli-${directory}.tgz`),
      );
    }
    const { stdout } = await npmPack(
      [
        join(packagesRoot, "cli"),
        "--ignore-scripts",
        "--pack-destination",
        packedRoot,
      ],
      {
        env: {
          ...process.env,
          npm_config_cache: join(temporary, "npm-cache"),
        },
      },
    );
    await rename(
      join(packedRoot, stdout.trim().split("\n").at(-1)),
      join(packedRoot, "polygl-cli.tgz"),
    );
    await verifyPackedRelease(
      "1.2.3-beta.1",
      packagesRoot,
      packedRoot,
    );
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
});
