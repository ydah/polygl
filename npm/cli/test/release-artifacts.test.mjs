import assert from "node:assert/strict";
import {
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import test from "node:test";

import {
  LEGAL_FILES,
  NATIVE_PACKAGES,
} from "../../scripts/prepare-release.mjs";
import { verifyReleaseArtifacts } from "../../scripts/verify-release-artifacts.mjs";

async function fixture() {
  const root = await mkdtemp(join(tmpdir(), "polygl-release-artifacts-"));
  const artifacts = join(root, "artifacts");
  const legal = new Map();
  const archives = new Map();
  for (const file of LEGAL_FILES) {
    const contents = Buffer.from(`${file}\n`);
    legal.set(file, contents);
    await writeFile(join(root, file), contents);
  }
  for (const entry of NATIVE_PACKAGES) {
    const binary = Buffer.from(`${entry.target}\n`);
    const binaryDirectory = join(artifacts, "bundle", entry.directory);
    await mkdir(binaryDirectory, { recursive: true });
    await writeFile(join(binaryDirectory, entry.binary), binary);

    const archive = join(
      artifacts,
      "release",
      `polygl-1.2.3-${entry.target}.tar.gz`,
    );
    await mkdir(join(artifacts, "release"), { recursive: true });
    await writeFile(archive, "");
    archives.set(basename(archive), new Map([[entry.binary, binary], ...legal]));
  }
  return { root, artifacts, archives };
}

test("accepts exactly five legal-complete native artifacts", async () => {
  const { root, artifacts, archives } = await fixture();
  try {
    await verifyReleaseArtifacts(
      "1.2.3",
      artifacts,
      root,
      async (archive) => {
        const contents = archives.get(basename(archive));
        return {
          entries: [...contents.keys()],
          read: async (entry) => contents.get(entry),
        };
      },
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("rejects an archive without all legal files", async () => {
  const { root, artifacts, archives } = await fixture();
  try {
    const firstArchive = archives.values().next().value;
    firstArchive.delete(LEGAL_FILES[0]);
    await assert.rejects(
      verifyReleaseArtifacts(
        "1.2.3",
        artifacts,
        root,
        async (archive) => {
          const contents = archives.get(basename(archive));
          return {
            entries: [...contents.keys()],
            read: async (entry) => contents.get(entry),
          };
        },
      ),
      /contains/,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("rejects a Windows archive with CRLF-converted legal text", async () => {
  const { root, artifacts, archives } = await fixture();
  try {
    const firstArchive = archives.values().next().value;
    firstArchive.set(LEGAL_FILES[0], Buffer.from(`${LEGAL_FILES[0]}\r\n`));
    await assert.rejects(
      verifyReleaseArtifacts(
        "1.2.3",
        artifacts,
        root,
        async (archive) => {
          const contents = archives.get(basename(archive));
          return {
            entries: [...contents.keys()],
            read: async (entry) => contents.get(entry),
          };
        },
      ),
      /outdated LICENSE-MIT/,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
