import { createGetUrl } from "fumadocs-core/source";

import releases from "../../../../.release-please-manifest.json";

export const siteName = "CIHub";
/** The version of the last release, that release-please writes. */
export const version = releases["."];
export const siteUrl = "https://cihub.io";
export const siteDescription =
  "CIHub is an open-source, self-hosted merge queue for GitHub. It keeps the target branch green with the CI you already have, runs as one container and one Postgres, and explains each of its decisions.";
export const repositoryUrl = "https://github.com/getcihub/cihub";
export const changelogUrl = `${repositoryUrl}/blob/main/CHANGELOG.md`;

export const docsRoute = "/docs";
export const docsContentRoute = "/llms.mdx/docs";
export const docsImageRoute = "/og/docs";

export const configSchemaPath = "/schema/config/v1.json";
export const openApiPath = "/openapi/v1.json";

const getContentUrl = createGetUrl(docsContentRoute);
const getImageUrl = createGetUrl(docsImageRoute);

/** The internal route that renders a page as Markdown; `proxy.ts` maps the public URLs onto it. */
export function getPageContentUrl(page: { slugs: string[] }) {
  const segments = [...page.slugs, "content.md"];

  return { segments, url: getContentUrl(segments) };
}

/** The public Markdown URL of a page: its URL with `.md`, `/docs.md` for the index. */
export function getPageMarkdownUrl(page: { url: string }) {
  return `${page.url}.md`;
}

export function getPageImageUrl(page: { slugs: string[] }) {
  const segments = [...page.slugs, "image.png"];

  return { segments, url: getImageUrl(segments) };
}
