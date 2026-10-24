import { docsLlms, getPagesInOrder, source } from "@/lib/source";
import { getPageContentUrl, getPageMarkdownUrl } from "@/lib/shared";

export const revalidate = false;

export async function GET(_request: Request, { params }: RouteContext<"/llms.mdx/docs/[[...slug]]">) {
  const { slug } = await params;
  const page = source.getPage(slug?.slice(0, -1));
  if (!page) {
    return pageNotFound();
  }

  return new Response(await docsLlms.page(page), {
    headers: { "Content-Type": "text/markdown; charset=utf-8" },
  });
}

// An agent that guessed a URL gets the list of the real ones, and corrects itself.
function pageNotFound() {
  const pages = getPagesInOrder().map((page) => `- ${getPageMarkdownUrl(page)}: ${page.data.title}`);
  const body = [
    "# Page not found",
    "",
    "No page of the CIHub documentation has this URL. Use one of these, as they are:",
    "",
    ...pages,
    "",
    "The index of the site is /llms.txt, and /llms-full.txt holds every page in one file.",
    "",
  ].join("\n");

  return new Response(body, {
    status: 404,
    headers: { "Content-Type": "text/markdown; charset=utf-8" },
  });
}

export function generateStaticParams() {
  return source.getPages().map((page) => ({ slug: getPageContentUrl(page).segments }));
}
