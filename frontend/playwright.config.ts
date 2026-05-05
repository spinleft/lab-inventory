import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  projects: [
    {
      name: "desktop-chromium",
      use: { ...devices["Desktop Chrome"] },
    },
    {
      name: "tablet-chromium",
      use: { ...devices["iPad Mini"], browserName: "chromium" },
    },
    {
      name: "mobile-chromium",
      use: { ...devices["Pixel 5"], browserName: "chromium" },
    },
  ],
  testDir: "./tests/e2e",
  use: {
    baseURL: "http://127.0.0.1:5173",
    trace: "on-first-retry",
  },
  webServer: {
    command: "npm run dev",
    reuseExistingServer: !process.env.CI,
    url: "http://127.0.0.1:5173",
  },
});
