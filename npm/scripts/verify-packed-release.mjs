import { readFile, stat } from "node:fs/promises";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import {
  LEGAL_FILES,
  NATIVE_PACKAGES,
  validateVersion,
} from "./prepare-release.mjs";
import { defaultArchiveReader } from "./verify-release-artifacts.mjs";

export const PACKED_PACKAGES = Object.freeze([
  ...NATIVE_PACKAGES.map((entry) => ({
    archive: `polygl-cli-${entry.directory}.tgz`,
    directory: join("platforms", entry.directory),
    files: [`bin/${entry.binary}`, ...LEGAL_FILES],
    name: entry.name,
    os: entry.os,
    cpu: entry.cpu,
  })),
  {
    archive: "polygl-cli.tgz",
    directory: "cli",
    files: ["bin/polygl.js", "lib/platform.mjs", ...LEGAL_FILES],
    name: "@polygl/cli",
  },
]);

function sameArray(actual, expected) {
  return (
    Array.isArray(actual) &&
    actual.length === expected.length &&
    actual.every((entry, index) => entry === expected[index])
  );
}

export function validateInstallabilityMetadata(manifest, expected) {
  if (expected.os) {
    if (!sameArray(manifest.os, [expected.os])) {
      throw new Error(`${expected.name} must target only ${expected.os}`);
    }
    if (!sameArray(manifest.cpu, [expected.cpu])) {
      throw new Error(`${expected.name} must target only ${expected.cpu}`);
    }
    return;
  }
  const bin = manifest.bin;
  if (
    typeof bin !== "object" ||
    bin === null ||
    Object.keys(bin).length !== 1 ||
    bin.polygl !== "bin/polygl.js"
  ) {
    throw new Error("@polygl/cli must expose bin.polygl as bin/polygl.js");
  }
}

function sameEntries(actual, expected) {
  const sortedActual = [...actual].sort();
  const sortedExpected = [...expected].sort();
  return (
    sortedActual.length === sortedExpected.length &&
    sortedActual.every((entry, index) => entry === sortedExpected[index])
  );
}

export async function verifyPackedRelease(
  version,
  packagesArgument,
  packedArgument,
  archiveReader = defaultArchiveReader,
) {
  validateVersion(version);
  const packagesRoot = resolve(packagesArgument);
  const packedRoot = resolve(packedArgument);

  for (const entry of PACKED_PACKAGES) {
    const archive = join(packedRoot, entry.archive);
    await stat(archive);
    const contents = await archiveReader(archive);
    const expectedEntries = ["package/package.json", ...entry.files.map(
      (file) => `package/${file}`,
    )];
    if (!sameEntries(contents.entries, expectedEntries)) {
      throw new Error(
        `${entry.archive} contains an unexpected package file set`,
      );
    }

    const manifest = JSON.parse(
      (await contents.read("package/package.json")).toString("utf8"),
    );
    if (manifest.name !== entry.name) {
      throw new Error(
        `${entry.archive} has package name ${manifest.name}; expected ${entry.name}`,
      );
    }
    if (manifest.version !== version) {
      throw new Error(
        `${entry.archive} has version ${manifest.version}; expected ${version}`,
      );
    }
    validateInstallabilityMetadata(manifest, entry);
    if (entry.name === "@polygl/cli") {
      for (const native of NATIVE_PACKAGES) {
        if (manifest.optionalDependencies?.[native.name] !== version) {
          throw new Error(
            `${entry.archive} does not pin ${native.name} to ${version}`,
          );
        }
      }
    }

    for (const file of entry.files) {
      const [packed, staged] = await Promise.all([
        contents.read(`package/${file}`),
        readFile(join(packagesRoot, entry.directory, file)),
      ]);
      if (!packed.equals(staged)) {
        throw new Error(`${entry.archive} contains an outdated ${file}`);
      }
    }
  }

  console.log(`verified ${PACKED_PACKAGES.length} packed npm release artifacts`);
}

async function main() {
  const [version, packagesRoot, packedRoot] = process.argv.slice(2);
  if (!version || !packagesRoot || !packedRoot) {
    throw new Error(
      "usage: verify-packed-release.mjs <version> <packages-root> <packed-root>",
    );
  }
  await verifyPackedRelease(version, packagesRoot, packedRoot);
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  await main();
}
