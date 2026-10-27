import pluginQuery from "@tanstack/eslint-plugin-query";
import pluginRouter from "@tanstack/eslint-plugin-router";
import { defineConfig } from "eslint/config";
import { readdirSync } from "node:fs";
import tseslint from "typescript-eslint";

import { base } from "../../eslint.config.base.js";

// The layers of src/ (AGENTS.md): pages → components → hooks → domain → lib. An import goes down,
// never up or sideways to another page, and an import from another folder starts with `@/`.
const fromAnotherFolder = { regex: "^\\.\\./", message: "Import another folder through `@/`." };
const ORDER = "the layers go pages → components → hooks → domain → lib";

function layer(files, name, forbidden) {
  const patterns = forbidden.map((folder) =>
    typeof folder === "string"
      ? { group: [`@/${folder}/*`], message: `${name} does not import @/${folder}: ${ORDER}.` }
      : folder,
  );
  return {
    files,
    rules: { "no-restricted-imports": ["error", { patterns: [fromAnotherFolder, ...patterns] }] },
  };
}

// A page imports its own folder, never the folder of another page.
const pageLayers = readdirSync(`${import.meta.dirname}/src/pages`, { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map(({ name }) =>
    layer([`src/pages/${name}/**`], "A page", [
      {
        regex: `^@/pages/(?!${name}/)`,
        message: "A page does not import another page: move the shared part to @/components or @/hooks.",
      },
    ]),
  );

export default defineConfig(
  { ignores: ["dist/", "src/client/", "src/routeTree.gen.ts"] },
  ...base,
  pluginQuery.configs["flat/recommended"],
  pluginRouter.configs["flat/recommended"],
  {
    languageOptions: {
      parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname },
    },
  },
  layer(["src/**"], "The console", []),
  ...pageLayers,
  layer(["src/components/**"], "A component", ["pages"]),
  layer(["src/hooks/**"], "A hook", ["pages", "components"]),
  layer(["src/domain/**"], "The domain", [
    "pages",
    "components",
    "hooks",
    { group: ["react"], message: "The domain has no React: a hook goes in @/hooks." },
  ]),
  layer(["src/lib/**"], "A library", ["pages", "components", "hooks", "domain", "client"]),
  // This file is JavaScript, outside the TypeScript project.
  { files: ["eslint.config.js"], extends: [tseslint.configs.disableTypeChecked] },
);
