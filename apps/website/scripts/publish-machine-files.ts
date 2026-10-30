// Copies the machine-readable files of the repository into public/, where the
// site serves them, and writes the list of the error codes that the page of the
// API errors includes: at build time, so that the repository keeps a single
// source for each of them.
import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

interface OpenApiDocument {
  servers?: { url: string }[];
  components: {
    schemas: { ErrorCode: { enum: string[] } };
    examples: Record<string, { value: { title: string; status: number; detail: string } }>;
  };
}

const repository = join(import.meta.dirname, "..", "..", "..");
const publicDirectory = join(import.meta.dirname, "..", "public");
const generatedDirectory = join(import.meta.dirname, "..", "generated");

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

// The order of `ErrorCode` groups the codes by status; the example of a code
// holds its status and the sentence that explains it.
const sections = new Map<string, string[]>();
for (const code of openapi.components.schemas.ErrorCode.enum) {
  const example = openapi.components.examples[code]?.value;
  if (!example) throw new Error(`the OpenAPI document has no example of the error code ${code}`);

  const heading = `## ${String(example.status)} ${example.title}`;
  sections.set(heading, [...(sections.get(heading) ?? []), `### \`${code}\`\n\n${example.detail}`]);
}
const errorCodes = [...sections].map(([heading, codes]) => [heading, ...codes].join("\n\n"));
await mkdir(generatedDirectory, { recursive: true });
await writeFile(join(generatedDirectory, "error-codes.md"), `${errorCodes.join("\n\n")}\n`);
