// The lint rules of every TypeScript package; a package adds the rules of its own stack and structure.
import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import tseslint from "typescript-eslint";

export const base = [
  js.configs.recommended,
  tseslint.configs.strictTypeChecked,
  tseslint.configs.stylisticTypeChecked,
  reactHooks.configs.flat["recommended-latest"],
  {
    rules: {
      // `any` is already an error (`strictTypeChecked`); `unknown` hides a type as well. A value whose type cannot
      // be known (a thrown value, a JSON of no schema) keeps it with a disable comment that says why.
      "no-restricted-syntax": [
        "error",
        {
          selector: "TSUnknownKeyword",
          message: "Name the type; `unknown` only where it cannot be known, with a comment that says why.",
        },
      ],
    },
  },
];
