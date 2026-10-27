import { DocsLayout } from "fumadocs-ui/layouts/notebook";

import { baseOptions } from "@/lib/layout.shared";
import { version } from "@/lib/shared";
import { source } from "@/lib/source";

export default function Layout({ children }: LayoutProps<"/docs">) {
  const base = baseOptions();

  return (
    <DocsLayout
      tree={source.getPageTree()}
      {...base}
      nav={{ ...base.nav, mode: "top" }}
      sidebar={{
        banner: (
          <div className="flex items-center gap-2 rounded-md border bg-fd-card px-2.5 py-2 text-sm">
            <span className="size-1.5 rounded-full bg-fd-warning" />v{version}
            <span className="text-fd-muted-foreground ms-auto">Early days</span>
          </div>
        ),
      }}
    >
      {children}
    </DocsLayout>
  );
}
