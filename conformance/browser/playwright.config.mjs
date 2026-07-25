import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: ".",
  testMatch: "render.spec.mjs",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: "line",
  timeout: 60_000,
  use: {
    browserName: "chromium",
    headless: true,
    launchOptions: {
      args: [
        "--enable-unsafe-swiftshader",
        "--enable-webgl",
        "--ignore-gpu-blocklist",
        "--use-angle=swiftshader-webgl",
        "--use-gl=angle",
      ],
    },
    viewport: { width: 320, height: 240 },
  },
});
