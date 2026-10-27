import { flattenTree } from "fumadocs-core/page-tree";
import { llms, loader } from "fumadocs-core/source";
import { metaSchema, pageSchema } from "fumadocs-core/source/schema";
import { applyMdxPreset } from "fumadocs-mdx/config";
import { defineDocs } from "fumadocs-mdx/macro";

import { remarkDropTitle } from "./remark-drop-title";
import { docsRoute, getPageMarkdownUrl, siteUrl } from "./shared";

const docs = defineDocs({
  dir: "content/docs",
  docs: {
    schema: pageSchema,
    // First, before the plugins that read the headings: the table of contents.
    mdxOptions: applyMdxPreset({ remarkPlugins: (plugins) => [remarkDropTitle, ...plugins] }),
    postprocess: {
      // An agent has no use for the explicit `[#id]` of each heading.
      includeProcessedMarkdown: { headingIds: false },
    },
  },
  meta: {
    schema: metaSchema,
  },
});

export const source = loader({
  baseUrl: docsRoute,
  source: docs.toFumadocsSource(),
});

/** The pages in the order of the navigation, which the meta.json files give. */
export function getPagesInOrder() {
  const urls = flattenTree(source.getPageTree().children).map((item) => item.url);

  return source.getPages().toSorted((a, b) => urls.indexOf(a.url) - urls.indexOf(b.url));
}

// JSON strings are valid YAML scalars: a title with a colon stays one value.
export const docsLlms = llms(source, {
  renderPage: async (page) => `---
title: ${JSON.stringify(page.data.title)}
description: ${JSON.stringify(page.data.description ?? "")}
canonical_url: ${JSON.stringify(`${siteUrl}${page.url}`)}
markdown_url: ${JSON.stringify(`${siteUrl}${getPageMarkdownUrl(page)}`)}
---

# ${page.data.title}

${await page.data.getText("processed")}`,
});
