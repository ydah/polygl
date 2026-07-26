import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import {
  chmod,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
const repositoryRoot = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../../..",
);

async function writeCommand(path, source) {
  await writeFile(path, `#!/usr/bin/env node\n${source}`);
  await chmod(path, 0o755);
}

async function fixture(t, curlMode) {
  const root = await mkdtemp(join(tmpdir(), "polygl-release-scripts-"));
  const state = join(root, "state");
  const commands = join(root, "commands.jsonl");
  await mkdir(state);
  await writeFile(commands, "");
  t.after(() => rm(root, { recursive: true, force: true }));

  const curl = join(root, "curl");
  await writeCommand(
    curl,
    `
import { appendFileSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
const url = process.argv.at(-1);
const mode = process.env.FAKE_CURL_MODE;
let status = mode === "present" ? "200" : mode === "error" ? "503" : "404";
if (mode === "sequence") {
  const key = Buffer.from(url).toString("base64url");
  const path = join(process.env.FAKE_STATE, key);
  const count = existsSync(path) ? Number(readFileSync(path, "utf8")) : 0;
  writeFileSync(path, String(count + 1));
  status = count === 0 ? "404" : "200";
}
appendFileSync(
  process.env.FAKE_COMMAND_LOG,
  JSON.stringify({ command: "curl", url, status }) + "\\n",
);
process.stdout.write(status);
`,
  );

  const command = join(root, "command");
  await writeCommand(
    command,
    `
import { appendFileSync } from "node:fs";
appendFileSync(
  process.env.FAKE_COMMAND_LOG,
  JSON.stringify({ command: process.env.FAKE_COMMAND_NAME, args: process.argv.slice(2) }) + "\\n",
);
`,
  );
  const sleep = join(root, "sleep");
  await writeCommand(
    sleep,
    `
import { appendFileSync } from "node:fs";
appendFileSync(
  process.env.FAKE_COMMAND_LOG,
  JSON.stringify({ command: "sleep", args: process.argv.slice(2) }) + "\\n",
);
`,
  );

  return {
    root,
    commands,
    env: {
      ...process.env,
      FAKE_COMMAND_LOG: commands,
      FAKE_CURL_MODE: curlMode,
      FAKE_STATE: state,
      RELEASE_CARGO_COMMAND: command,
      RELEASE_CURL_COMMAND: curl,
      RELEASE_NPM_COMMAND: command,
      RELEASE_SLEEP_COMMAND: sleep,
    },
  };
}

async function run(script, args, env) {
  return execFileAsync("bash", [script, ...args], {
    cwd: repositoryRoot,
    env,
  });
}

async function readCommands(path) {
  return (await readFile(path, "utf8"))
    .split("\n")
    .filter(Boolean)
    .map(JSON.parse);
}

async function expectedCrates() {
  return (await readFile(
    join(repositoryRoot, "scripts/release-crate-stages.txt"),
    "utf8",
  )).trim().split(/\s+/);
}

async function expectedCrateStages() {
  return (await readFile(
    join(repositoryRoot, "scripts/release-crate-stages.txt"),
    "utf8",
  )).trim().split("\n").map((stage) => stage.split(/\s+/));
}

test("publishes missing crates in dependency-stage order", async (t) => {
  const context = await fixture(t, "sequence");
  await run(
    "scripts/release-crates.sh",
    ["publish", "1.2.3"],
    { ...context.env, FAKE_COMMAND_NAME: "cargo" },
  );
  const commands = await readCommands(context.commands);
  const published = commands
    .filter(({ command }) => command === "cargo")
    .map(({ args }) => args.at(-1));
  assert.deepEqual(published, await expectedCrates());
  assert.equal(commands.some(({ command }) => command === "sleep"), false);

  const stages = await expectedCrateStages();
  for (let index = 0; index < stages.length - 1; index += 1) {
    const current = stages[index];
    const next = new Set(stages[index + 1]);
    const lastPublish = commands.findLastIndex(
      ({ command, args }) => command === "cargo" && current.includes(args.at(-1)),
    );
    const nextPublish = commands.findIndex(
      ({ command, args }) => command === "cargo" && next.has(args.at(-1)),
    );
    for (const crate of current) {
      const visible = commands.findIndex(
        ({ command, url, status }, eventIndex) =>
          eventIndex > lastPublish &&
          eventIndex < nextPublish &&
          command === "curl" &&
          status === "200" &&
          url.includes(`/crates/${crate}/1.2.3`),
      );
      assert.notEqual(visible, -1, `${crate} was not visible before the next stage`);
    }
  }
});

test("skips crates whose exact version exists", async (t) => {
  const context = await fixture(t, "present");
  await run(
    "scripts/release-crates.sh",
    ["publish", "1.2.3"],
    { ...context.env, FAKE_COMMAND_NAME: "cargo" },
  );
  assert.equal(
    (await readCommands(context.commands)).some(
      ({ command }) => command === "cargo",
    ),
    false,
  );
});

test("aborts crate publication on a registry error", async (t) => {
  const context = await fixture(t, "error");
  await assert.rejects(
    run(
      "scripts/release-crates.sh",
      ["publish", "1.2.3"],
      { ...context.env, FAKE_COMMAND_NAME: "cargo" },
    ),
  );
  assert.equal(
    (await readCommands(context.commands)).some(
      ({ command }) => command === "cargo",
    ),
    false,
  );
});

test("bounds crates.io propagation polling", async (t) => {
  const context = await fixture(t, "missing");
  await assert.rejects(
    run(
      "scripts/release-crates.sh",
      ["publish", "1.2.3"],
      {
        ...context.env,
        CRATES_IO_POLL_ATTEMPTS: "2",
        CRATES_IO_POLL_INTERVAL_SECONDS: "0",
        FAKE_COMMAND_NAME: "cargo",
      },
    ),
  );
  const commands = await readCommands(context.commands);
  assert.equal(
    commands.filter(({ command }) => command === "cargo").length,
    1,
  );
  assert.equal(
    commands.filter(({ command }) => command === "sleep").length,
    1,
  );
});

const npmArchives = [
  "polygl-cli-darwin-arm64.tgz",
  "polygl-cli-darwin-x64.tgz",
  "polygl-cli-linux-arm64.tgz",
  "polygl-cli-linux-x64.tgz",
  "polygl-cli-win32-x64.tgz",
  "polygl-cli.tgz",
];

async function stageNpmArchives(root) {
  const packages = join(root, "packages");
  await mkdir(packages);
  await Promise.all(
    npmArchives.map((archive) => writeFile(join(packages, archive), "")),
  );
  return packages;
}

for (const distTag of ["latest", "next"]) {
  test(`publishes native npm packages before the launcher with ${distTag}`, async (t) => {
    const context = await fixture(t, "sequence");
    const packages = await stageNpmArchives(context.root);
    await run(
      "scripts/release-npm.sh",
      ["1.2.3", distTag, packages],
      { ...context.env, FAKE_COMMAND_NAME: "npm" },
    );
    const commands = await readCommands(context.commands);
    const publishes = commands.filter(({ command }) => command === "npm");
    const archives = publishes.map(({ args }) => args[1].split("/").at(-1));
    assert.deepEqual(archives, npmArchives);
    for (const { args } of publishes) {
      assert.ok(args.includes("--provenance"));
      assert.equal(args[args.indexOf("--tag") + 1], distTag);
    }

    const launcherPublish = commands.findIndex(
      ({ command, args }) =>
        command === "npm" && args[1].endsWith("/polygl-cli.tgz"),
    );
    for (const archive of npmArchives.slice(0, -1)) {
      const packageName = `@polygl/${archive.slice(7, -4)}`;
      const nativePublish = commands.findIndex(
        ({ command, args }) =>
          command === "npm" && args[1].endsWith(`/${archive}`),
      );
      const visible = commands.findIndex(
        ({ command, url, status }, eventIndex) =>
          eventIndex > nativePublish &&
          eventIndex < launcherPublish &&
          command === "curl" &&
          status === "200" &&
          url.includes(packageName.replace("/", "%2F")),
      );
      assert.notEqual(visible, -1, `${packageName} was not visible before launcher`);
    }
  });
}

test("skips npm packages whose exact version exists", async (t) => {
  const context = await fixture(t, "present");
  const packages = await stageNpmArchives(context.root);
  await run(
    "scripts/release-npm.sh",
    ["1.2.3", "latest", packages],
    { ...context.env, FAKE_COMMAND_NAME: "npm" },
  );
  assert.equal(
    (await readCommands(context.commands)).some(
      ({ command }) => command === "npm",
    ),
    false,
  );
});

test("aborts npm publication on a registry error", async (t) => {
  const context = await fixture(t, "error");
  const packages = await stageNpmArchives(context.root);
  await assert.rejects(
    run(
      "scripts/release-npm.sh",
      ["1.2.3", "latest", packages],
      { ...context.env, FAKE_COMMAND_NAME: "npm" },
    ),
  );
  assert.equal(
    (await readCommands(context.commands)).some(
      ({ command }) => command === "npm",
    ),
    false,
  );
});

test("bounds npm registry propagation polling", async (t) => {
  const context = await fixture(t, "missing");
  const packages = await stageNpmArchives(context.root);
  await assert.rejects(
    run(
      "scripts/release-npm.sh",
      ["1.2.3", "latest", packages],
      {
        ...context.env,
        FAKE_COMMAND_NAME: "npm",
        NPM_POLL_ATTEMPTS: "2",
        NPM_POLL_INTERVAL_SECONDS: "0",
      },
    ),
  );
  const commands = await readCommands(context.commands);
  assert.equal(
    commands.filter(({ command }) => command === "npm").length,
    5,
  );
  assert.equal(
    commands.filter(({ command }) => command === "sleep").length,
    1,
  );
});
