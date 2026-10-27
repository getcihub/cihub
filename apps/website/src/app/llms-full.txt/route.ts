import { docsLlms, getPagesInOrder } from "@/lib/source";

export const revalidate = false;

export async function GET() {
  const pages = await Promise.all(getPagesInOrder().map((page) => docsLlms.page(page)));

  return new Response(pages.join("\n\n"), {
    headers: { "Content-Type": "text/markdown; charset=utf-8" },
  });
}
