import { chmod, copyFile, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const npmRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = resolve(npmRoot, "..");

export const LEGAL_FILES = Object.freeze([
  "LICENSE-MIT",
  "LICENSE-APACHE",
  "THIRD_PARTY_LICENSES.txt",
]);

export const NATIVE_PACKAGES = Object.freeze([
  {
    directory: "darwin-arm64",
    name: "@polygl/cli-darwin-arm64",
    binary: "polygl",
  },
  {
    directory: "darwin-x64",
    name: "@polygl/cli-darwin-x64",
    binary: "polygl",
  },
  {
    directory: "linux-arm64",
    name: "@polygl/cli-linux-arm64",
    binary: "polygl",
  },
  {
    directory: "linux-x64",
    name: "@polygl/cli-linux-x64",
    binary: "polygl",
  },
  {
    directory: "win32-x64",
    name: "@polygl/cli-win32-x64",
    binary: "polygl.exe",
  },
]);

function validateVersion(version) {
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
    throw new Error(`release version must be a SemVer value without a v prefix: ${version}`);
  }
}

async function readPackage(path) {
  return JSON.parse(await readFile(path, "utf8"));
}

async function writePackage(path, manifest) {
  await writeFile(path, `${JSON.stringify(manifest, null, 2)}\n`);
}

async function copyLegalFiles(packageRoot, legalRoot) {
  await Promise.all(
    LEGAL_FILES.map((file) =>
      copyFile(join(legalRoot, file), join(packageRoot, file)),
    ),
  );
}

export async function prepareRelease(
  version,
  artifactsRoot,
  packagesRoot = npmRoot,
  legalRoot = repositoryRoot,
) {
  validateVersion(version);

  for (const entry of NATIVE_PACKAGES) {
    const packageRoot = join(packagesRoot, "platforms", entry.directory);
    const manifestPath = join(packageRoot, "package.json");
    const manifest = await readPackage(manifestPath);
    if (manifest.name !== entry.name) {
      throw new Error(`unexpected package name in ${manifestPath}: ${manifest.name}`);
    }
    manifest.version = version;
    await writePackage(manifestPath, manifest);

    const source = join(artifactsRoot, "bundle", entry.directory, entry.binary);
    const destination = join(packageRoot, "bin", entry.binary);
    await copyFile(source, destination);
    await rm(join(packageRoot, "bin", ".gitkeep"), { force: true });
    if (entry.binary === "polygl") {
      await chmod(destination, 0o755);
    }
    await copyLegalFiles(packageRoot, legalRoot);
  }

  const launcherRoot = join(packagesRoot, "cli");
  const launcherPath = join(launcherRoot, "package.json");
  const launcher = await readPackage(launcherPath);
  launcher.version = version;
  for (const entry of NATIVE_PACKAGES) {
    if (!(entry.name in launcher.optionalDependencies)) {
      throw new Error(`launcher is missing optional dependency ${entry.name}`);
    }
    launcher.optionalDependencies[entry.name] = version;
  }
  await writePackage(launcherPath, launcher);
  await copyLegalFiles(launcherRoot, legalRoot);
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [version, artifactArgument] = process.argv.slice(2);
  if (!version || !artifactArgument) {
    console.error("usage: node npm/scripts/prepare-release.mjs <version> <artifacts-directory>");
    process.exit(2);
  }
  await prepareRelease(version, resolve(artifactArgument));
}
