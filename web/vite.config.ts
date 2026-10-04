import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// The local cihub server of `mise run dev` (CIHUB_HTTP_ADDR).
const server = "http://127.0.0.1:8080";

export default defineConfig({
  plugins: [react()],
  server: {
    proxy: { "/api": server },
  },
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.{ts,tsx}"],
    setupFiles: ["src/test-setup.ts"],
  },
});
