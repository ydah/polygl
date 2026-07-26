import { execFile } from "node:child_process";
import { readFile, readdir, stat } from "node:fs/promises";
import { basename, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { promisify } from "node:util";

import {
  LEGAL_FILES,
  NATIVE_PACKAGES,
  validateVersion,
} from "./prepare-release.mjs";

const execFileAsync = promisify(execFile);

export async function defaultArchiveReader(archive) {
  const { stdout } = await execFileAsync("tar", ["-tzf", archive], {
    maxBuffer: 16 * 1024 * 1024,
  });
  const entries = stdout
    .split("\n")
    .filter(Boolean)
    .map((entry) => entry.replace(/^\.\//, ""));
  return {
    entries,
    read: async (entry) =>
      (
        await execFileAsync("tar", ["-xOzf", archive, entry], {
          encoding: null,
          maxBuffer: 16 * 1024 * 1024,
        })
      ).stdout,
  };
}

function sameEntries(actual, expected) {
  return (
    actual.length === expected.length &&
    [...actual].sort().every((entry, index) => entry === [...expected].sort()[index])
  );
}

export async function verifyReleaseArtifacts(
  version,
  artifactsArgument,
  repositoryArgument = ".",
  archiveReader = defaultArchiveReader,
) {
  validateVersion(version);
  const artifactsRoot = resolve(artifactsArgument);
  const repositoryRoot = resolve(repositoryArgument);
  const expectedArchives = [];

  for (const entry of NATIVE_PACKAGES) {
    const binary = join(
      artifactsRoot,
      "bundle",
      entry.directory,
      entry.binary,
    );
    await stat(binary);

    const archive = join(
      artifactsRoot,
      "release",
      `polygl-${version}-${entry.target}.tar.gz`,
    );
    await stat(archive);
    expectedArchives.push(basename(archive));

    const contents = await archiveReader(archive);
    const expectedEntries = [entry.binary, ...LEGAL_FILES];
    if (!sameEntries(contents.entries, expectedEntries)) {
      throw new Error(
        `${basename(archive)} contains ${contents.entries.join(", ")}; expected ${expectedEntries.join(", ")}`,
      );
    }
    const archivedBinary = await contents.read(entry.binary);
    if (!archivedBinary.equals(await readFile(binary))) {
      throw new Error(`${basename(archive)} contains a different native binary`);
    }
    for (const legalFile of LEGAL_FILES) {
      const archivedLegalFile = await contents.read(legalFile);
      const repositoryLegalFile = await readFile(
        join(repositoryRoot, legalFile),
      );
      if (!archivedLegalFile.equals(repositoryLegalFile)) {
        throw new Error(
          `${basename(archive)} contains an outdated ${legalFile}`,
        );
      }
    }
  }

  const actualArchives = (await readdir(join(artifactsRoot, "release"))).filter(
    (entry) => entry.endsWith(".tar.gz"),
  );
  if (!sameEntries(actualArchives, expectedArchives)) {
    throw new Error("release artifacts do not match the five supported targets");
  }

  console.log(`verified ${NATIVE_PACKAGES.length} native release artifacts`);
}

async function main() {
  const [version, artifactsRoot, repositoryRoot = "."] = process.argv.slice(2);
  if (!version || !artifactsRoot) {
    throw new Error(
      "usage: verify-release-artifacts.mjs <version> <artifacts-root> [repository-root]",
    );
  }
  await verifyReleaseArtifacts(version, artifactsRoot, repositoryRoot);
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  await main();
}
