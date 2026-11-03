import { fileURLToPath } from "node:url";

import tailwindcss from "@tailwindcss/vite";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// The development server sends the API, the sign-in and the webhooks to a
// local `cihub-server start` (CIHUB_URL, http://localhost:8080 by default).
const server = process.env.CIHUB_URL ?? "http://localhost:8080";

// `mise run dev` sets CIHUB_TUNNEL_URL: the tunnel runs in a container, which
// reaches Vite only on an interface other than loopback, and a person reaches
// Vite at the public address of the instance. Vite then accepts that host, and
// the page opens its reload socket on the port of that address.
const isBehindTunnel = process.env.CIHUB_TUNNEL_URL !== undefined;
const publicUrl =
  isBehindTunnel && process.env.CIHUB_HTTP_PUBLIC_URL
    ? new URL(process.env.CIHUB_HTTP_PUBLIC_URL)
    : undefined;

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
  // The maps travel in the binary with the bundles: a Sentry server that reaches the instance reads them there.
  build: { sourcemap: true },
  server: {
    proxy: {
      "/api": server,
      "/auth": server,
      "/webhooks": server,
    },
    host: isBehindTunnel ? "0.0.0.0" : undefined,
    allowedHosts: publicUrl ? [publicUrl.hostname] : [],
    hmr: publicUrl?.protocol === "https:" ? { clientPort: 443 } : undefined,
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
