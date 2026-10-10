import { tanstackRouter } from "@tanstack/router-plugin/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// The development server sends the API and the sign-in to a local
// `cihub server start` (CIHUB_URL, http://localhost:8080 by default).
const server = process.env.CIHUB_URL ?? "http://localhost:8080";

export default defineConfig({
  plugins: [tanstackRouter({ target: "react", autoCodeSplitting: true }), react()],
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
  },
});
