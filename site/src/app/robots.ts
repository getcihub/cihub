import type { MetadataRoute } from "next";

import { siteUrl } from "@/lib/shared";

// Every crawler, search engine or AI agent, reads every page.
export default function robots(): MetadataRoute.Robots {
  return {
    rules: { userAgent: "*", allow: "/", disallow: "/api/" },
    sitemap: `${siteUrl}/sitemap.xml`,
  };
}
