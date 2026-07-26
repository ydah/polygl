#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";

import { resolveBinary } from "../lib/platform.mjs";

let binary;
try {
  binary = resolveBinary(createRequire(import.meta.url).resolve);
} catch (error) {
  console.error(`polygl: ${error.message}`);
  process.exit(1);
}

const result = spawnSync(binary, process.argv.slice(2), {
  stdio: "inherit",
  windowsHide: false,
});

if (result.error) {
  console.error(`polygl: failed to start native executable: ${result.error.message}`);
  process.exit(1);
}
if (result.signal) {
  console.error(`polygl: native executable terminated by ${result.signal}`);
  process.exit(1);
}
process.exit(result.status ?? 1);
