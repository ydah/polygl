import { defineConfig } from "@playwright/test";
import {
  swiftShaderLaunchOptions,
  viewport,
} from "./swiftshader.mjs";

export default defineConfig({
  testDir: ".",
  testMatch: "webgl-stability.spec.mjs",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: "line",
  use: {
    browserName: "chromium",
    headless: true,
    launchOptions: swiftShaderLaunchOptions,
    viewport,
  },
});
