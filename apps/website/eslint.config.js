import { defineConfig } from "eslint/config";
import tseslint from "typescript-eslint";

import { base } from "../../eslint.config.base.js";

export default defineConfig(
  { ignores: [".next/", ".source/", "next-env.d.ts", "public/"] },
  ...base,
  {
    languageOptions: {
      parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname },
    },
  },
  // node:test runs each `describe` and `test` itself: nothing awaits their promises.
  { files: ["tests/**"], rules: { "@typescript-eslint/no-floating-promises": "off" } },
  // These files are JavaScript, outside the TypeScript project.
  { files: ["*.js", "*.mjs"], extends: [tseslint.configs.disableTypeChecked] },
);
