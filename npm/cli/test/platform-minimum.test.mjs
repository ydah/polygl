import assert from "node:assert/strict";
import { chmod, mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { delimiter, join } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";

const script = fileURLToPath(
  new URL("../../../scripts/check-platform-minimum.sh", import.meta.url),
);

async function mockCommand(directory, name, body) {
  const path = join(directory, name);
  await writeFile(path, `#!/usr/bin/env bash\n${body}\n`);
  await chmod(path, 0o755);
}

function run(platform, directory, extra = {}) {
  return spawnSync(script, [join(directory, "binary"), platform], {
    encoding: "utf8",
    env: {
      ...process.env,
      PATH: `${directory}${delimiter}${process.env.PATH}`,
      ...extra,
    },
  });
}

test("platform floor checks accept the boundary and reject newer requirements", async () => {
  const directory = await mkdtemp(join(tmpdir(), "polygl-platform-floor-"));
  await writeFile(join(directory, "binary"), "fixture");

  await mockCommand(
    directory,
    "readelf",
    "printf '%s\\n' 'Name: GLIBC_2.17' 'Name: GLIBC_2.39'",
  );
  assert.equal(run("linux", directory).status, 0);
  await mockCommand(directory, "readelf", "printf '%s\\n' 'Name: GLIBC_2.40'");
  const linuxFailure = run("linux", directory);
  assert.equal(linuxFailure.status, 1);
  assert.match(linuxFailure.stderr, /newer than supported 2\.39/);

  await mockCommand(directory, "otool", "printf '%s\\n' '      minos 11.0'");
  assert.equal(run("macos", directory).status, 0);
  await mockCommand(directory, "otool", "printf '%s\\n' '      minos 12.0'");
  assert.equal(run("macos", directory).status, 1);

  await mockCommand(directory, "file", "printf '%s\\n' 'fixture: PE32+ executable Windows'");
  assert.equal(run("windows", directory).status, 0);
  await mockCommand(directory, "file", "printf '%s\\n' 'fixture: ELF executable'");
  assert.notEqual(run("windows", directory).status, 0);
  assert.equal(run("unknown", directory).status, 2);
});
