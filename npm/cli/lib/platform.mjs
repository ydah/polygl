export const PLATFORM_PACKAGES = Object.freeze({
  "darwin-arm64": "@polygl/cli-darwin-arm64",
  "darwin-x64": "@polygl/cli-darwin-x64",
  "linux-arm64": "@polygl/cli-linux-arm64",
  "linux-x64": "@polygl/cli-linux-x64",
  "win32-x64": "@polygl/cli-win32-x64",
});

export function selectPackage(platform = process.platform, arch = process.arch) {
  const key = `${platform}-${arch}`;
  const packageName = PLATFORM_PACKAGES[key];
  if (!packageName) {
    const supported = Object.keys(PLATFORM_PACKAGES).sort().join(", ");
    throw new Error(`unsupported platform ${key}; supported platforms: ${supported}`);
  }
  return packageName;
}

export function binaryName(platform = process.platform) {
  return platform === "win32" ? "polygl.exe" : "polygl";
}

export function resolveBinary(
  resolve,
  platform = process.platform,
  arch = process.arch,
) {
  const packageName = selectPackage(platform, arch);
  const request = `${packageName}/bin/${binaryName(platform)}`;
  try {
    return resolve(request);
  } catch (cause) {
    throw new Error(
      `native package ${packageName} is unavailable; make sure npm optional dependencies were installed for ${platform}-${arch}`,
      { cause },
    );
  }
}
