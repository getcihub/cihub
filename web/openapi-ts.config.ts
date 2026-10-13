// The client of the API, generated from the document that the server serves
// (`mise run generate`): never edited by hand.
import { defineConfig } from "@hey-api/openapi-ts";

export default defineConfig({
  input: "./openapi.json",
  output: "src/client",
  plugins: ["@hey-api/client-fetch", "@tanstack/react-query"],
});
