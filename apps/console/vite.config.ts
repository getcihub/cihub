import { fileURLToPath } from "node:url";

import tailwindcss from "@tailwindcss/vite";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// The development server sends the API and the sign-in to a local
// `cihub-server start` (CIHUB_URL, http://localhost:8080 by default).
const server = process.env.CIHUB_URL ?? "http://localhost:8080";

export default defineConfig({
  plugins: [
    tanstackRouter({
      target: "react",
      autoCodeSplitting: true,
      // The tree of the addresses, and the route file of each page.
      virtualRouteConfig: "./src/routes.ts",
      routesDirectory: "./src/pages",
    }),
    react(),
    tailwindcss(),
  ],
  // `@/` is `src/`: an import from another folder names where it comes from.
  resolve: { alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) } },
  server: {
    proxy: {
      "/api": server,
      "/auth": server,
    },
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    // The browser tests of e2e/ run with Playwright.
    include: ["src/**/*.test.{ts,tsx}"],
    reporters: ["default", "junit"],
    outputFile: { junit: "../../test-reports/console-unit/junit.xml" },
  },
});
