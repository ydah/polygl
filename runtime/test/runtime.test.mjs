import assert from "node:assert/strict";
import test from "node:test";

import { runtimeOps, runtimeVersion } from "../dist/index.js";

test("exports the runtime skeleton", () => {
  assert.deepEqual(runtimeOps, {});
  assert.equal(runtimeVersion, "0.0.0");
});
