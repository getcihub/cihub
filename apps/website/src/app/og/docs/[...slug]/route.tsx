import { generateOGImage } from "fumadocs-ui/og";
import { notFound } from "next/navigation";

import { getPageImageUrl, siteName } from "@/lib/shared";
import { source } from "@/lib/source";

export const revalidate = false;

export async function GET(_request: Request, { params }: RouteContext<"/og/docs/[...slug]">) {
  const { slug } = await params;
  const page = source.getPage(slug.slice(0, -1));
  if (!page) notFound();

  return generateOGImage({
    title: page.data.title,
    description: page.data.description,
    site: siteName,
  });
}

export function generateStaticParams() {
  return source.getPages().map((page) => ({ slug: getPageImageUrl(page).segments }));
}
