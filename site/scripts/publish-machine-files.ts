// Copies the machine-readable files of the repository into public/, where the
// site serves them: a copy at build time, so that the repository keeps a single
// source for each of them.
import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

interface OpenApiDocument {
  servers?: { url: string }[];
}

const repository = join(import.meta.dirname, "..", "..");
const publicDirectory = join(import.meta.dirname, "..", "public");

await mkdir(join(publicDirectory, "schema", "config"), { recursive: true });
await copyFile(
  join(repository, "schema", "config", "v1.json"),
  join(publicDirectory, "schema", "config", "v1.json"),
);

// The relative server `/api/v1` is right when an instance serves the document,
// wrong on cihub.io, which hosts no API.
const openapi = JSON.parse(
  await readFile(join(repository, "schema", "openapi", "v1.json"), "utf8"),
) as OpenApiDocument;
openapi.servers = openapi.servers?.filter((server) => !server.url.startsWith("/"));
await mkdir(join(publicDirectory, "openapi"), { recursive: true });
await writeFile(join(publicDirectory, "openapi", "v1.json"), `${JSON.stringify(openapi, null, 2)}\n`);
