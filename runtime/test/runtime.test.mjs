import assert from "node:assert/strict";
import test from "node:test";

import { runtimeOps, runtimeVersion } from "../dist/index.js";

test("exports the runtime skeleton", () => {
  assert.equal(runtimeOps.background, "background");
  assert.equal(runtimeOps.no_stroke, "noStroke");
  assert.equal(runtimeOps.time, "time");
  assert.equal(runtimeVersion, "0.0.0");
});
