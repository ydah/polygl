import { execFile } from "node:child_process";
import { appendFile, readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { promisify } from "node:util";

import { validateVersion } from "./prepare-release.mjs";

const execFileAsync = promisify(execFile);

function readWorkspaceVersion(manifest) {
  let inWorkspacePackage = false;
  for (const line of manifest.split("\n")) {
    const trimmed = line.trim();
    if (trimmed.startsWith("[") && trimmed.endsWith("]")) {
      inWorkspacePackage = trimmed === "[workspace.package]";
      continue;
    }
    const version = inWorkspacePackage
      ? trimmed.match(/^version\s*=\s*"([^"]+)"$/)?.[1]
      : undefined;
    if (version) {
      return version;
    }
  }
  throw new Error("Cargo.toml is missing workspace.package.version");
}

function isPublishable(packageMetadata) {
  return (
    packageMetadata.publish === null ||
    (Array.isArray(packageMetadata.publish) &&
      packageMetadata.publish.length > 0)
  );
}

export function validateRelease(candidate, manifest, metadata) {
  validateVersion(candidate);
  const workspaceVersion = readWorkspaceVersion(manifest);
  validateVersion(workspaceVersion);
  if (candidate !== workspaceVersion) {
    throw new Error(
      `release version ${candidate} does not match workspace version ${workspaceVersion}`,
    );
  }

  const members = new Set(metadata.workspace_members);
  const workspacePackages = metadata.packages.filter((packageMetadata) =>
    members.has(packageMetadata.id),
  );
  for (const packageMetadata of workspacePackages) {
    if (packageMetadata.version !== workspaceVersion) {
      throw new Error(
        `${packageMetadata.name} has version ${packageMetadata.version}; expected ${workspaceVersion}`,
      );
    }
  }
  const publishable = workspacePackages.filter(isPublishable);
  if (publishable.length === 0) {
    throw new Error("Cargo workspace has no publishable packages");
  }

  const prerelease = candidate.split("+", 1)[0].includes("-");
  return {
    version: candidate,
    prerelease,
    distTag: prerelease ? "next" : "latest",
    crates: publishable.map(({ name }) => name).sort(),
  };
}

export function validatePublishStages(crates, stages) {
  const listed = stages
    .split("\n")
    .flatMap((stage) => stage.trim().split(/\s+/))
    .filter(Boolean)
    .sort();
  if (new Set(listed).size !== listed.length) {
    throw new Error("release crate stages contain a duplicate package");
  }
  if (
    listed.length !== crates.length ||
    listed.some((crate, index) => crate !== [...crates].sort()[index])
  ) {
    throw new Error(
      `release crate stages do not match publishable workspace crates: ${listed.join(", ")}`,
    );
  }
}

async function main() {
  const [candidate, repositoryArgument = "."] = process.argv.slice(2);
  if (!candidate) {
    throw new Error("usage: validate-release.mjs <version> [repository-root]");
  }
  const repositoryRoot = resolve(repositoryArgument);
  const [manifest, stages, metadataResult] = await Promise.all([
    readFile(resolve(repositoryRoot, "Cargo.toml"), "utf8"),
    readFile(
      resolve(repositoryRoot, "scripts/release-crate-stages.txt"),
      "utf8",
    ),
    execFileAsync(
      "cargo",
      ["metadata", "--format-version", "1", "--no-deps"],
      { cwd: repositoryRoot, maxBuffer: 16 * 1024 * 1024 },
    ),
  ]);
  const result = validateRelease(
    candidate,
    manifest,
    JSON.parse(metadataResult.stdout),
  );
  validatePublishStages(result.crates, stages);

  if (process.env.GITHUB_OUTPUT) {
    await appendFile(
      process.env.GITHUB_OUTPUT,
      [
        `version=${result.version}`,
        `prerelease=${result.prerelease}`,
        `dist-tag=${result.distTag}`,
        "",
      ].join("\n"),
    );
  }
  console.log(JSON.stringify(result, null, 2));
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  await main();
}
