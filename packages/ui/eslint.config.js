import { defineConfig } from "eslint/config";
import tseslint from "typescript-eslint";

import { base } from "../../eslint.config.base.js";

export default defineConfig(
  ...base,
  {
    languageOptions: {
      parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname },
    },
  },
  // This file is JavaScript, outside the TypeScript project.
  { files: ["eslint.config.js"], extends: [tseslint.configs.disableTypeChecked] },
);
