import { fileURLToPath } from "node:url";

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// The viewer of `mise run console:shots-diff`, built as one script and one style sheet. A page opened from the disk
// (`file://`) runs no module script, so the build is an IIFE rather than the modules of the dashboard.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: { alias: { "@": fileURLToPath(new URL("../../../src", import.meta.url)) } },
  define: { "process.env.NODE_ENV": JSON.stringify("production") },
  publicDir: false,
  build: {
    lib: {
      entry: fileURLToPath(new URL("main.tsx", import.meta.url)),
      name: "VisualChanges",
      formats: ["iife"],
      fileName: () => "viewer.js",
      cssFileName: "viewer",
    },
  },
});
