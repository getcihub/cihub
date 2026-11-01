import { isMarkdownPreferred, rewritePath } from "fumadocs-core/negotiation";
import { type NextRequest, NextResponse } from "next/server";

import { docsContentRoute, docsRoute } from "@/lib/shared";

const markdownSuffix = rewritePath(`${docsRoute}{/*path}.md`, `${docsContentRoute}{/*path}/content.md`);
const docsPage = rewritePath(`${docsRoute}{/*path}`, `${docsContentRoute}{/*path}/content.md`);

// These URLs have two representations, selected by `Accept`: a cache that
// ignores it would serve Markdown to a browser. Next.js sets the `Vary` of an
// HTML page itself, so only the Markdown response carries `Accept`.
const negotiated = { headers: { Vary: "Accept" } };

export default function proxy(request: NextRequest) {
  const { pathname } = request.nextUrl;

  // next.config.ts sets skipTrailingSlashRedirect for PostHog: a page of the
  // documentation keeps one URL here.
  if (pathname.length > 1 && pathname.endsWith("/")) {
    const url = new URL(request.url);
    url.pathname = pathname.slice(0, -1);
    return NextResponse.redirect(url, 308);
  }

  const markdownPage = markdownSuffix.rewrite(pathname);
  if (markdownPage) {
    return NextResponse.rewrite(new URL(markdownPage, request.nextUrl));
  }

  const markdownRepresentation = pathname === "/" ? "/llms.txt" : docsPage.rewrite(pathname);
  if (!markdownRepresentation || !isMarkdownPreferred(request)) {
    return NextResponse.next();
  }

  return NextResponse.rewrite(new URL(markdownRepresentation, request.nextUrl), negotiated);
}

export const config = {
  matcher: ["/", "/docs", "/docs/:path*", "/docs.md"],
};
