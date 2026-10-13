import js from "@eslint/js";
import pluginQuery from "@tanstack/eslint-plugin-query";
import pluginRouter from "@tanstack/eslint-plugin-router";
import reactHooks from "eslint-plugin-react-hooks";
import { defineConfig } from "eslint/config";
import tseslint from "typescript-eslint";

// The layers of src/ (web/AGENTS.md): routes → pages → components → domain → ui and lib. An import goes down, never up or
// sideways to another page, and an import from another folder starts with `@/`.
const fromAnotherFolder = { regex: "^\\.\\./", message: "Import another folder through `@/`." };

function layer(files, name, forbidden) {
  const patterns = forbidden.map((folder) => ({
    group: [`@/${folder}/*`],
    message: `${name} does not import @/${folder}: the layers go routes → pages → components → domain → ui, lib.`,
  }));
  return {
    files,
    rules: { "no-restricted-imports": ["error", { patterns: [fromAnotherFolder, ...patterns] }] },
  };
}

export default defineConfig(
  { ignores: ["dist/", "src/client/", "src/routeTree.gen.ts", "e2e/__screenshots__/"] },
  js.configs.recommended,
  tseslint.configs.strictTypeChecked,
  tseslint.configs.stylisticTypeChecked,
  reactHooks.configs.flat["recommended-latest"],
  pluginQuery.configs["flat/recommended"],
  pluginRouter.configs["flat/recommended"],
  {
    languageOptions: {
      parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname },
    },
  },
  layer(["src/**"], "The dashboard", []),
  layer(["src/routes/**"], "A route", ["routes"]),
  layer(["src/pages/**"], "A page", ["routes", "pages"]),
  layer(["src/components/**"], "A component", ["routes", "pages"]),
  layer(["src/domain/**"], "The domain", ["routes", "pages", "components"]),
  layer(["src/ui/**"], "The UI kit", ["routes", "pages", "components", "domain", "client"]),
  layer(["src/lib/**"], "A library", ["routes", "pages", "components", "domain", "client", "ui"]),
  // This file is JavaScript, outside the TypeScript project.
  { files: ["eslint.config.js"], extends: [tseslint.configs.disableTypeChecked] },
);
