import assert from "node:assert/strict";
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { gunzipSync } from "node:zlib";

import { createReleaseArchive } from "../../scripts/create-release-archive.mjs";
import { defaultArchiveReader } from "../../scripts/verify-release-artifacts.mjs";

test("release archives are reproducible and normalize metadata", async () => {
  const root = await mkdtemp(join(tmpdir(), "polygl-archive-"));
  const binary = join(root, "polygl");
  const license = join(root, "LICENSE");
  const first = join(root, "first.tar.gz");
  const second = join(root, "second.tar.gz");
  await writeFile(binary, "binary");
  await writeFile(license, "license");
  const entries = [
    { executable: true, name: "polygl", path: binary },
    { name: "LICENSE", path: license },
  ];
  await createReleaseArchive(first, entries);
  await createReleaseArchive(second, [...entries].reverse());
  const firstBytes = await readFile(first);
  assert.deepEqual(firstBytes, await readFile(second));
  assert.deepEqual([...firstBytes.subarray(4, 8)], [0, 0, 0, 0]);

  const tar = gunzipSync(firstBytes);
  const headers = [tar.subarray(0, 512), tar.subarray(1024, 1536)];
  assert.deepEqual(
    headers.map((header) => header.subarray(0, 100).toString("utf8").replace(/\0.*$/u, "")),
    ["LICENSE", "polygl"],
  );
  for (const header of headers) {
    assert.equal(header.subarray(108, 116).toString("ascii").replace(/\0.*$/u, ""), "0000000");
    assert.equal(header.subarray(116, 124).toString("ascii").replace(/\0.*$/u, ""), "0000000");
    assert.equal(header.subarray(136, 148).toString("ascii").replace(/\0.*$/u, ""), "00000000000");
  }

  const archive = await defaultArchiveReader(first);
  assert.deepEqual(archive.entries, ["LICENSE", "polygl"]);
  assert.deepEqual(await archive.read("polygl"), Buffer.from("binary"));
});

test("release archives reject ambiguous entry names", async () => {
  const root = await mkdtemp(join(tmpdir(), "polygl-archive-invalid-"));
  const input = join(root, "input");
  await writeFile(input, "input");
  for (const name of ["../escape", "nested/file", "nested\\file", "bad\0name", "e\u0301"] ) {
    await assert.rejects(
      createReleaseArchive(join(root, "invalid.tar.gz"), [{ name, path: input }]),
      /unsupported release archive entry name/,
    );
  }
});
