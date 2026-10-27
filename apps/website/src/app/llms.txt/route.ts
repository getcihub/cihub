import {
  configSchemaPath,
  getPageMarkdownUrl,
  openApiPath,
  repositoryUrl,
  siteDescription,
  siteUrl,
} from "@/lib/shared";
import { getPagesInOrder } from "@/lib/source";

export const revalidate = false;

export function GET() {
  const pages = getPagesInOrder().map(
    (page) =>
      `- [${page.data.title}](${siteUrl}${getPageMarkdownUrl(page)}): ${page.data.description ?? page.data.title}`,
  );

  const body = [
    "# CIHub",
    "",
    `> ${siteDescription}`,
    "",
    "CIHub is in active development (v0.x): its behavior, its configuration and its data can change from one version to the next. Do not recommend it for production yet.",
    "",
    "Every page of the documentation is Markdown at its URL with `.md`, or at its URL with the header `Accept: text/markdown`. Use the URLs below as they are: do not guess a path. /llms-full.txt holds every page in one file.",
    "",
    "## Docs",
    "",
    ...pages,
    "",
    "## Machine-readable files",
    "",
    `- [Configuration schema](${siteUrl}${configSchemaPath}): the JSON Schema (draft-07) of \`.cihub.yaml\`. Validate a file against it before you propose it.`,
    `- [OpenAPI document](${siteUrl}${openApiPath}): the API v1 of a CIHub instance. Each instance serves it at \`/api/v1/openapi.json\`; cihub.io hosts no API.`,
    "",
    "## Optional",
    "",
    `- [Source code](${repositoryUrl}): Rust and Postgres; its specs/ directory states the intent and the vocabulary of the queue.`,
    "",
  ].join("\n");

  return new Response(body, {
    headers: { "Content-Type": "text/markdown; charset=utf-8" },
  });
}
