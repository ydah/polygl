import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const binary = process.argv[2] ?? join(root, "target", "release", "polygl");
const iterations = Number.parseInt(process.env.POLYGL_BENCH_ITERATIONS ?? "5", 10);
if (!Number.isSafeInteger(iterations) || iterations < 1 || iterations > 100) {
  throw new Error("POLYGL_BENCH_ITERATIONS must be an integer from 1 to 100");
}

const cases = JSON.parse(
  readFileSync(join(root, "benchmarks", "compiler", "cases.json"), "utf8"),
);
const results = [];

for (const benchmark of cases) {
  const samples = [];
  for (let iteration = 0; iteration < iterations; iteration += 1) {
    const output = mkdtempSync(join(tmpdir(), `polygl-${benchmark.id}-`));
    const started = process.hrtime.bigint();
    const result = spawnSync(
      binary,
      ["build", resolve(root, benchmark.source), "--release", "-o", output],
      { encoding: "utf8" },
    );
    const elapsedMs = Number(process.hrtime.bigint() - started) / 1_000_000;
    rmSync(output, { force: true, recursive: true });
    const succeeded = result.status === 0;
    if (succeeded !== benchmark.expectSuccess) {
      throw new Error(
        `${benchmark.id} returned ${String(result.status)}: ${result.stderr}`,
      );
    }
    samples.push(elapsedMs);
  }
  samples.sort((left, right) => left - right);
  results.push({
    id: benchmark.id,
    iterations,
    medianMs: samples[Math.floor(samples.length / 2)],
    minMs: samples[0],
    maxMs: samples.at(-1),
    source: benchmark.source,
  });
}

process.stdout.write(`${JSON.stringify({ schemaVersion: 1, results }, null, 2)}\n`);
