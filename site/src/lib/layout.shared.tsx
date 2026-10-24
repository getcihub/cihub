import type { BaseLayoutProps } from "fumadocs-ui/layouts/shared";

import { Mark } from "@/components/mark";

import { changelogUrl, repositoryUrl, siteName } from "./shared";

export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      title: (
        <>
          <Mark className="size-6" />
          {siteName}
          <span className="text-fd-muted-foreground font-normal">Docs</span>
        </>
      ),
    },
    githubUrl: repositoryUrl,
    links: [
      { text: "Product", url: "/", active: "url" },
      { text: "Docs", url: "/docs", active: "nested-url" },
      { text: "Changelog", url: changelogUrl, external: true },
    ],
  };
}
