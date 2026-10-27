import { buttonVariants } from "fumadocs-ui/components/ui/button";
import {
  DocsBody,
  DocsDescription,
  DocsPage,
  DocsTitle,
  EditOnGitHub,
  MarkdownCopyButton,
} from "fumadocs-ui/layouts/notebook/page";
import { createRelativeLink } from "fumadocs-ui/mdx";
import type { Metadata } from "next";
import { notFound } from "next/navigation";

import { JsonLd } from "@/components/json-ld";
import { getMDXComponents } from "@/components/mdx";
import { getPageImageUrl, getPageMarkdownUrl, repositoryUrl, siteName, siteUrl } from "@/lib/shared";
import { source } from "@/lib/source";

export default async function Page(props: PageProps<"/docs/[[...slug]]">) {
  const params = await props.params;
  const page = source.getPage(params.slug);
  if (!page) notFound();

  const MDX = page.data.body;
  const markdownUrl = getPageMarkdownUrl(page);

  return (
    <DocsPage
      toc={page.data.toc}
      full={page.data.full}
      tableOfContent={{
        footer: (
          <div className="bg-fd-card text-fd-muted-foreground mt-6 grid gap-1.5 rounded-lg border p-3 text-xs">
            <span className="text-fd-foreground text-sm font-medium">For agents</span>
            This page as Markdown:
            <a href={markdownUrl} className="text-fd-primary font-mono">
              {markdownUrl}
            </a>
          </div>
        ),
      }}
    >
      <JsonLd
        data={{
          "@context": "https://schema.org",
          "@type": "TechArticle",
          headline: page.data.title,
          description: page.data.description,
          url: `${siteUrl}${page.url}`,
          isPartOf: { "@type": "WebSite", name: siteName, url: siteUrl },
        }}
      />
      <DocsTitle className="text-4xl tracking-tight">{page.data.title}</DocsTitle>
      <DocsDescription className="mb-0">{page.data.description}</DocsDescription>
      <div className="flex flex-row flex-wrap items-center gap-2 border-b pb-6 *:shrink-0">
        <MarkdownCopyButton markdownUrl={markdownUrl}>Copy as Markdown</MarkdownCopyButton>
        <a href={markdownUrl} className={buttonVariants({ variant: "secondary", size: "sm" })}>
          Open the .md
        </a>
        <EditOnGitHub href={`${repositoryUrl}/blob/main/site/content/docs/${page.path}`} />
      </div>
      <DocsBody>
        <MDX components={getMDXComponents({ a: createRelativeLink(source, page) })} />
      </DocsBody>
    </DocsPage>
  );
}

export function generateStaticParams() {
  return source.generateParams();
}

export async function generateMetadata(props: PageProps<"/docs/[[...slug]]">): Promise<Metadata> {
  const params = await props.params;
  const page = source.getPage(params.slug);
  if (!page) notFound();

  return {
    title: page.data.title,
    description: page.data.description,
    alternates: {
      canonical: page.url,
      // Tells an agent that reads the HTML where the Markdown of the page is.
      types: { "text/markdown": getPageMarkdownUrl(page) },
    },
    openGraph: { url: page.url, images: getPageImageUrl(page).url },
  };
}
