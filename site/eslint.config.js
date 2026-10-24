import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import { defineConfig } from "eslint/config";
import tseslint from "typescript-eslint";

export default defineConfig(
  { ignores: [".next/", ".source/", "next-env.d.ts", "public/"] },
  js.configs.recommended,
  tseslint.configs.strictTypeChecked,
  tseslint.configs.stylisticTypeChecked,
  reactHooks.configs.flat["recommended-latest"],
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
