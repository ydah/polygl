import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: ".",
  testMatch: "portability.spec.mjs",
  fullyParallel: false,
  workers: 1,
  retries: 1,
  reporter: "line",
  timeout: 60_000,
  projects: [
    { name: "firefox", use: { browserName: "firefox", headless: true } },
    { name: "webkit", use: { browserName: "webkit", headless: true } },
  ],
  use: {
    viewport: { width: 320, height: 240 },
  },
});
