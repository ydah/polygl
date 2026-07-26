import assert from "node:assert/strict";
import test from "node:test";

import {
  validatePublishStages,
  validateRelease,
} from "../../scripts/validate-release.mjs";

const manifest = `
[workspace]
members = ["public", "private"]

[workspace.package]
version = "1.2.3-beta.1"
`;

const metadata = {
  workspace_members: ["public-id", "private-id"],
  packages: [
    {
      id: "public-id",
      name: "public",
      version: "1.2.3-beta.1",
      publish: null,
    },
    {
      id: "private-id",
      name: "private",
      version: "1.2.3-beta.1",
      publish: [],
    },
  ],
};

test("validates the coordinated prerelease version", () => {
  assert.deepEqual(validateRelease("1.2.3-beta.1", manifest, metadata), {
    version: "1.2.3-beta.1",
    prerelease: true,
    distTag: "next",
    crates: ["public"],
  });
});

test("uses latest for stable releases", () => {
  const stableManifest = manifest.replace("1.2.3-beta.1", "1.2.3");
  const stableMetadata = structuredClone(metadata);
  for (const packageMetadata of stableMetadata.packages) {
    packageMetadata.version = "1.2.3";
  }
  assert.equal(
    validateRelease("1.2.3", stableManifest, stableMetadata).distTag,
    "latest",
  );
});

test("rejects invalid or mismatched release versions", () => {
  assert.throws(
    () => validateRelease("01.2.3", manifest, metadata),
    /must be a SemVer/,
  );
  assert.throws(
    () => validateRelease("1.2.3", manifest, metadata),
    /does not match workspace version/,
  );
  assert.throws(
    () => validateRelease("1.2.3-01", manifest, metadata),
    /must be a SemVer/,
  );
});

test("rejects a divergent publishable crate", () => {
  const divergentMetadata = structuredClone(metadata);
  divergentMetadata.packages[0].version = "1.2.4";
  assert.throws(
    () => validateRelease("1.2.3-beta.1", manifest, divergentMetadata),
    /public has version 1.2.4/,
  );
});

test("rejects a divergent private workspace member", () => {
  const divergentMetadata = structuredClone(metadata);
  divergentMetadata.packages[1].version = "1.2.4";
  assert.throws(
    () => validateRelease("1.2.3-beta.1", manifest, divergentMetadata),
    /private has version 1.2.4/,
  );
});

test("requires every publishable crate in the dependency stages once", () => {
  validatePublishStages(["public"], "public\n");
  assert.throws(
    () => validatePublishStages(["public"], "public\npublic\n"),
    /duplicate package/,
  );
  assert.throws(
    () => validatePublishStages(["public", "second"], "public\n"),
    /do not match/,
  );
});
